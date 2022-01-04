mod config;
mod log;
mod poller;
mod snapshots;

use crate::config::{Config, WatchConfig};
use std::process;

#[tokio::main]
async fn main() {
    let dir = std::env::current_dir().unwrap();
    match std::env::args().nth(1).as_deref() {
        Some("capture") => {
            if let Some(oid) = snapshots::capture(&dir).unwrap() {
                println!("{}", oid);
            }
        }
        Some("serve") => {
            println!("pid: {}", std::process::id());
            poller::start().await;
        }
        Some("watch") => {
            watch_dir(&dir);
        }
        Some("kill") => {
            kill();
        }
        val => {
            dbg!(&val);
            eprintln!("Usage: dura capture");
            process::exit(1);
        }
    }
}

fn watch_dir(path: &std::path::PathBuf) {
    let mut config = Config::load();
    config.set_watch(path.to_str().unwrap().to_string(), WatchConfig::new());
    config.save();
}


/// kills running dura poller
///
/// poller's check to make sure that their pid is the same as the pid
/// found in config, and if they are not the same they exit. This
/// function does not actually kill a poller but instead indicates
/// that any living poller should exit during their next check.
fn kill() {
    let mut config = Config::load();
    config.pid = None;
    config.save();
}
