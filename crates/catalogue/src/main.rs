use std::process::ExitCode;

use catalogue::{Cli, run};
use clap::Parser;

fn main() -> ExitCode {
    run(Cli::parse())
}
