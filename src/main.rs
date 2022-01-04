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

fn kill() {
    let mut config = Config::load();
    config.pid = None;
    config.save();
}
