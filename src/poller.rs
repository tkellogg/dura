use std::path::Path;
use std::process;
use std::time::Instant;

use tokio::time;
use tracing::{error, info};

use crate::config::Config;
use crate::log::Operation;
use crate::snapshots;

#[tracing::instrument]
fn process_directory(path: &Path) {
    let mut op: Option<snapshots::CaptureStatus> = None;
    let mut error: Option<String> = None;
    let start_time = Instant::now();

    match snapshots::capture(path) {
        Ok(Some(status)) => op = Some(status),
        Ok(None) => (),
        Err(err) => {
            error = Some(format!("{}", err));
        }
    }

    let latency = (Instant::now() - start_time).as_secs_f32();
    let repo = path.to_str().unwrap_or("<invalid path>").to_string();
    let operation = Operation::Snapshot {
        repo,
        op,
        error,
        latency,
    };
    if operation.should_log() {
        info!(operation = %serde_json::to_string(&operation).unwrap(),"info_operation")
    }
}

#[tracing::instrument]
fn do_task() {
    let config = Config::load();
    if config.pid != Some(process::id()) {
        error!(
            "Shutting down because other poller took lock: {:?}",
            config.pid
        );
        process::exit(1);
    }

    for (key, _value) in config.repos {
        let path = Path::new(key.as_str());
        process_directory(path);
    }
}

pub async fn start() {
    let mut config = Config::load();
    config.pid = Some(process::id());
    config.save();

    loop {
        time::sleep(time::Duration::from_secs(5)).await;
        do_task();
    }
}
