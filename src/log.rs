use std::fmt::Debug;

use crate::snapshots::CaptureStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Operation {
    Snapshot {
        repo: String,
        op: Option<CaptureStatus>,
        error: Option<String>,
        latency: f32,
    },
}

impl Operation {
    pub fn should_log(&self) -> bool {
        match self {
            Operation::Snapshot {
                repo: _,
                op,
                error,
                latency: _,
            } => op.is_some() || error.is_some(),
        }
    }
}
