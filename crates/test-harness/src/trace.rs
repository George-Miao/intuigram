//! Machine-readable behavior trace collection.

use std::fs;
use std::path::PathBuf;

use serde::Serialize;

use super::error::Artifact;

#[derive(Clone, Debug, Serialize)]
pub struct TraceStep {
    pub number: usize,
    pub kind: &'static str,
    pub detail: String,
    pub revision: u64,
}

#[derive(Debug, Serialize)]
pub struct Trace {
    name: String,
    virtual_time: String,
    seed: u64,
    terminal: (u16, u16),
    steps: Vec<TraceStep>,
    final_screen: Vec<String>,
    pending_work: Vec<String>,
}

impl Trace {
    pub fn new(
        name: impl Into<String>,
        virtual_time: impl Into<String>,
        seed: u64,
        terminal: (u16, u16),
    ) -> Self {
        Self {
            name: name.into(),
            virtual_time: virtual_time.into(),
            seed,
            terminal,
            steps: Vec::new(),
            final_screen: Vec::new(),
            pending_work: Vec::new(),
        }
    }

    pub fn record(&mut self, kind: &'static str, detail: impl Into<String>, revision: u64) {
        self.steps.push(TraceStep {
            number: self.steps.len() + 1,
            kind,
            detail: detail.into(),
            revision,
        });
    }

    pub fn update_screen(&mut self, rows: Vec<String>) {
        self.final_screen = rows;
    }

    pub fn set_pending(&mut self, pending: Vec<String>) {
        self.pending_work = pending;
    }

    pub fn persist(&self) -> Artifact {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let directory = manifest.join("../../target/intuigram-test-traces");
        let safe_name = self
            .name
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                    character
                } else {
                    '_'
                }
            })
            .collect::<String>();
        let path = directory.join(format!("{safe_name}-{}.json", self.seed));
        let written = fs::create_dir_all(&directory)
            .and_then(|()| serde_json::to_vec_pretty(self).map_err(std::io::Error::other))
            .and_then(|bytes| fs::write(&path, bytes));
        if written.is_ok() {
            Artifact(Some(path))
        } else {
            Artifact(None)
        }
    }
}
