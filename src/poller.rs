use std::path::Path;
use std::process;
use std::time::Instant;

use tokio::time;

use crate::snapshots;
use crate::config::Config;
use crate::log::{Log, Logger, Operation};

fn process_directory(path: &Path) -> Log {
    let mut op: Option<snapshots::CaptureStatus> = None;
    let mut error: Option<String> = None;
    let start_time = Instant::now();

    match snapshots::capture(path) {
        Ok(Some(status)) => { op = Some(status) },
        Ok(None) => (),
        Err(err) => { error = Some(format!("{}", err)); },
    }

    let latency = (Instant::now() - start_time).as_secs_f32();
    let repo = path.to_str().unwrap_or("<invalid path>").to_string();
    Log::new(Operation::Snapshot{ repo, op, error, latency })
}

fn do_task() {
    let config = Config::load();
    if config.pid != Some(process::id()) {
        eprintln!("Shutting down because other poller took lock: {:?}", config.pid);
        process::exit(1);
    }

    let mut logger = Logger::new();
    for (key, _value) in config.repos {
        let path = Path::new(key.as_str());
        let log_op = process_directory(&path);
        logger.write(log_op);
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
