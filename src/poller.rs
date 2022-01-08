use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Instant;

use tokio::time;
use tracing::{error, info};

use crate::config::{Config, WatchConfig};
use crate::log::Operation;
use crate::snapshots;

/// Checks the provided `child_path` is a directory.
/// If either `includes` or `excludes` are set,
/// checks whether the path is included/excluded respectively.
fn is_valid_directory(base_path: &Path, child_path: &PathBuf, value: &WatchConfig) -> bool {
    if !child_path.is_dir() {
        return false;
    }

    let includes = &value.include;
    let excludes = &value.exclude;

    if includes.len() > 0 {
        let relative = child_path.strip_prefix(base_path).unwrap().to_str().unwrap();
        includes
            .iter()
            .find(|&include| include.starts_with(relative))
            .is_some()
    } else if excludes.len() > 0 {
        let relative = child_path.strip_prefix(base_path).unwrap().to_str().unwrap();
        excludes
            .iter()
            .find(|&exclude| exclude.starts_with(relative))
            .is_none()
    } else {
        true
    }
}

/// If the directory is a repo, attempts to create a snapshot.
/// Otherwise, recurses into each child directory.
#[tracing::instrument]
fn process_directory(base_path: &Path, current_path: &Path, value: &WatchConfig, depth: u8) {
    if snapshots::is_repo(current_path) {
        let mut op: Option<snapshots::CaptureStatus> = None;
        let mut error: Option<String> = None;
        let start_time = Instant::now();

        match snapshots::capture(current_path) {
            Ok(Some(status)) => op = Some(status),
            Ok(None) => (),
            Err(err) => {
                error = Some(format!("{}", err));
            }
        }

        let latency = (Instant::now() - start_time).as_secs_f32();
        let repo = current_path
            .to_str()
            .unwrap_or("<invalid path>")
            .to_string();
        let operation = Operation::Snapshot {
            repo,
            op,
            error,
            latency,
        };
        if operation.should_log() {
            info!(operation = %serde_json::to_string(&operation).unwrap(),"info_operation")
        }
    } else {
        if depth > value.max_depth {
            return;
        }

        let paths = fs::read_dir(current_path).unwrap();

        paths
            .filter_map(|entry| {
                let child_path = entry.unwrap().path();
                if is_valid_directory(base_path, &child_path, value) {
                    Some(child_path)
                } else {
                    None
                }
            })
            .for_each(|path| process_directory(base_path, path.as_path(), value, depth + 1));
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

    for (key, value) in config.repos {
        let path = Path::new(key.as_str());
        process_directory(path, path, &value, 0);
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
