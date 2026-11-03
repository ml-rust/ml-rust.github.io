//! A pacer that records the waits it is asked for.

use catalogue_signals::http::{Headers, Pacer};

/// A pacer that records the `retry-after` of each response it waits on.
#[derive(Default)]
pub struct RecordingPacer {
    pub waits: Vec<Option<String>>,
}

impl Pacer for RecordingPacer {
    fn wait(&mut self, response_headers: &Headers) {
        self.waits
            .push(response_headers.get("retry-after").map(str::to_owned));
    }
}
