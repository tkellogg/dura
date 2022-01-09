use std::fs;
use std::path::Path;
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
fn is_valid_directory(base_path: &Path, child_path: &Path, value: &WatchConfig) -> bool {
    if !child_path.is_dir() {
        return false;
    }

    let includes = &value.include;
    let excludes = &value.exclude;

    let mut include = true;

    if !excludes.is_empty() {
        include = !excludes
            .iter()
            .any(|exclude| child_path.starts_with(base_path.join(exclude)));
    }

    if !include && !includes.is_empty() {
        include = includes
            .iter()
            .any(|include| base_path.join(include).starts_with(child_path));
    }

    include
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
        if depth >= value.max_depth {
            return;
        }

        if let Ok(paths) = fs::read_dir(current_path) {
            paths
                .filter_map(|entry| {
                    if let Ok(entry) = entry {
                        let child_path = entry.path();
                        if is_valid_directory(base_path, &child_path, value) {
                            return Some(child_path);
                        }
                    }

                    None
                })
                .for_each(|path| process_directory(base_path, path.as_path(), value, depth + 1));
        }
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
