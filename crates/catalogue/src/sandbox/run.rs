//! Runs one `cargo check` in a probe project, with a time limit and a scrubbed environment.

use std::ffi::OsString;
use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use catalogue_signals::sandbox::Exit;

use super::error::SandboxCommandError;

/// The most bytes kept from each output stream. The rest is read and dropped.
const STREAM_LIMIT: u64 = 32 << 20;

/// The wait between checks of a running build.
const POLL: Duration = Duration::from_millis(250);

/// How long the output readers get after the build ends.
const DRAIN: Duration = Duration::from_secs(30);

/// What one `cargo check` produced.
#[derive(Debug)]
pub(super) struct Output {
    /// How cargo ended.
    pub exit: Exit,
    /// Cargo's JSON message lines.
    pub stdout: String,
    /// Cargo's error text. Only matched against fixed markers, never stored.
    pub stderr: String,
}

/// Runs `cargo check --message-format=json --quiet` in `dir`.
///
/// The process starts with an empty environment plus `env`, `CARGO_TARGET_DIR`
/// set to `<dir>/target`, and `CARGO_TERM_COLOR=never`. It runs in its own process
/// group. Past `timeout`, the whole group is killed and the exit reads as
/// [`Exit::TimedOut`]. The group is also killed after a normal exit, so a process
/// a build script leaves behind cannot touch the next check.
pub(super) fn cargo_check(
    dir: &Path,
    env: &[(&'static str, OsString)],
    timeout: Duration,
) -> Result<Output, SandboxCommandError> {
    let mut child = Command::new("cargo")
        .args(["check", "--message-format=json", "--quiet"])
        .current_dir(dir)
        .env_clear()
        .envs(env.iter().map(|(name, value)| (*name, value)))
        .env("CARGO_TARGET_DIR", dir.join("target"))
        .env("CARGO_TERM_COLOR", "never")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|source| SandboxCommandError::Spawn {
            program: "cargo",
            source,
        })?;
    let stdout = child.stdout.take().map(drain::<ChildStdout>);
    let stderr = child.stderr.take().map(drain::<ChildStderr>);
    let ended = wait(&mut child, timeout);
    kill_group(&mut child);
    let deadline = Instant::now().checked_add(DRAIN);
    let stdout = collect(stdout, deadline);
    let stderr = collect(stderr, deadline);
    let exit = match (ended, &stdout, &stderr) {
        (Ok(Some(true)), Some(_), Some(_)) => Exit::Success,
        (Ok(Some(false)), Some(_), Some(_)) => Exit::Failure,
        (Err(source), _, _) => {
            return Err(SandboxCommandError::Spawn {
                program: "cargo",
                source,
            });
        }
        _ => Exit::TimedOut,
    };
    Ok(Output {
        exit,
        stdout: stdout.unwrap_or_default(),
        stderr: stderr.unwrap_or_default(),
    })
}

/// Waits for `child` until `timeout`.
///
/// `Some(success)` when it ends in time, `None` when it does not.
fn wait(child: &mut Child, timeout: Duration) -> io::Result<Option<bool>> {
    let deadline = Instant::now().checked_add(timeout);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status.success()));
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Ok(None);
        }
        thread::sleep(POLL);
    }
}

/// Kills the process group of `child`, then `child` itself, and reaps it.
///
/// Errors are ignored: the group can already be gone.
fn kill_group(child: &mut Child) {
    let group = format!("-{}", child.id());
    let _ = Command::new("kill")
        .args(["-s", "KILL", "--", group.as_str()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
    let _ = child.wait();
}

/// Reads `stream` on its own thread: up to [`STREAM_LIMIT`] bytes kept, the rest dropped.
fn drain<R: Read + Send + 'static>(stream: R) -> Receiver<String> {
    let (send, receive) = mpsc::channel();
    thread::spawn(move || {
        let mut kept = Vec::new();
        let mut limited = stream.take(STREAM_LIMIT);
        let _ = limited.read_to_end(&mut kept);
        let _ = io::copy(&mut limited.into_inner(), &mut io::sink());
        let _ = send.send(String::from_utf8_lossy(&kept).into_owned());
    });
    receive
}

/// The text a [`drain`] thread read, or `None` when it does not finish by `deadline`.
///
/// A process that escapes the group can hold a pipe open. Its output is then lost.
fn collect(stream: Option<Receiver<String>>, deadline: Option<Instant>) -> Option<String> {
    let stream = stream?;
    match deadline {
        Some(deadline) => stream
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .ok(),
        None => stream.recv().ok(),
    }
}
