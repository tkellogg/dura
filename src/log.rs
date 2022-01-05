/// This file establishes a JSON logging mechanism to support `dura stat`.
use std::fs::{File, OpenOptions, create_dir_all};
use std::io;
use std::process;

use serde::{Deserialize, Serialize};
use chrono::{Utc, Datelike};

use crate::config::Config;
use crate::snapshots::CaptureStatus;

#[derive(Debug, Serialize, Deserialize)]
pub enum Operation {
    Snapshot { repo: String, op: Option<CaptureStatus>, error: Option<String>, latency: f32 },
}

impl Operation {
    fn should_log(&self) -> bool {
        !matches!(self, Operation::Snapshot { repo: _, op: None, error: None, latency: _ })
    }
}

/// Information to log to a log file. These are written in a plain-text structured format, like
/// JSON.
#[derive(Debug, Serialize, Deserialize)]
pub struct Log {
    time: String,
    op: Operation,
}

impl Log {
    pub fn new(op: Operation) -> Self {
        Self {
            time: Utc::now().to_rfc3339(),
            op,
        }
    }
}

/// For writing log lines out to a file
pub struct Logger {
    file: Option<File>,
}

impl Logger {
    pub fn new() -> Self {
        Self { file: None }
    }

    pub fn write(&mut self, log: Log) {
        if !log.op.should_log() {
            return
        }

        let _ = self.ensure_open();
        if let Some(ref file) = self.file {
            let writer = io::LineWriter::new(file);
            if let Err(error) = serde_json::to_writer(writer, &log) {
                eprintln!("Unable to write log: {:?}", error);
            }
        }
    }
    
    fn ensure_open(&mut self) -> bool {
        let now = Utc::now().date();
        let pid = process::id();
        let file_name = format!("logs/{}{}{}-{}-log.json", now.year(), now.month(), now.day(), pid);
        let path = Config::default_path().parent().unwrap().join(file_name);
        (&path).parent().map(create_dir_all);

        let file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(path.as_path());

        match file {
            Err(_) => {
                eprintln!("Unable to write file {}", path.to_str().unwrap_or("n/a"));
                false
            },
            Ok(file) => {
                self.file = Some(file);
                true
            },
        }
    }
}

impl Default for Logger {
    fn default() -> Self {
        Logger::new()
    }
}

