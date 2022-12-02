use std::path::Path;
use std::process;
use std::time::Instant;

use tokio::time;
use tracing::{debug, error, info, trace};

use crate::config::Config;
use crate::database::RuntimeLock;
use crate::log::{Operation, StatCollector};
use crate::poll_guard::PollGuard;
use crate::snapshots;

/// If the directory is a repo, attempts to create a snapshot.
/// Otherwise, recurses into each child directory.
#[tracing::instrument]
fn process_directory(current_path: &Path, guard: &mut PollGuard) {
    let mut op: Option<snapshots::CaptureStatus> = None;
    let mut error: Option<String> = None;
    let start_time = Instant::now();

    if guard.dir_changed(current_path) {
        debug!(
            "Potential change detected in repo: path = {path}",
            path = current_path.to_str().unwrap_or("")
        );
        match snapshots::capture(current_path) {
            Ok(Some(status)) => op = Some(status),
            Ok(None) => (),
            Err(err) => {
                error = Some(format!("{}", err));
            }
        }
    } else {
        trace!(
            "No files in repo have changed: path = {path}",
            path = current_path.to_str().unwrap_or("")
        );
    }

    let latency = (Instant::now() - start_time).as_secs_f32();
    let repo = current_path
        .to_str()
        .unwrap_or("<invalid path>")
        .to_string();
    let mut operation = Operation::Snapshot {
        repo,
        op,
        error,
        latency,
    };
    if operation.should_log() {
        info!(operation = operation.log_str().as_str(), "info_operation")
    }
}

#[tracing::instrument]
fn do_task(stats: &mut StatCollector, guard: &mut PollGuard) {
    let runtime_lock = RuntimeLock::load();
    if runtime_lock.pid != Some(process::id()) {
        error!(
            "Shutting down because other poller took lock: {:?}",
            runtime_lock.pid
        );
        process::exit(1);
    }

    let mut config = Config::load();

    let loop_start = Instant::now();
    for repo in config.git_repos() {
        let dir_start = Instant::now();
        process_directory(repo.as_path(), guard);
        stats.record_dir(Instant::now() - dir_start);
    }
    stats.record_loop(Instant::now() - loop_start);

    if stats.should_log() {
        info!(operation = stats.log_str().as_str(), "poller_stats");
    }
}

pub async fn start() {
    let mut runtime_lock = RuntimeLock::load();
    runtime_lock.pid = Some(process::id());
    runtime_lock.save();
    info!(pid = std::process::id());

    let mut stats = StatCollector::new();
    let mut guard = PollGuard::new();
    loop {
        time::sleep(time::Duration::from_secs(5)).await;
        do_task(&mut stats, &mut guard);
    }
}
