use dura::config::{Config, WatchConfig};
use dura::logger::NestedJsonLayer;
use dura::poller;
use dura::snapshots;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};
use clap::{App, AppSettings};

#[tokio::main]
async fn main() {
    let dir = std::env::current_dir().unwrap();

    let matches = App::new("dura")
        .about("Dura backs up your work automatically via Git commits.")
        .version("0.1.0")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .author("Tim Kellogg and the Internet")
        .subcommand(
            App::new("capture")
                .short_flag('C')
                .long_flag("capture")
                .about("Run a single backup of an entire repository. This is the one single iteration of the `serve` control loop.")
        )
        .subcommand(
            App::new("serve")
                .short_flag('S')
                .long_flag("serve")
                .about("Starts the worker that listens for file changes. If another process is already running, this will do it's best to terminate the other process.")
        )
        .subcommand(
            App::new("watch")
                .short_flag('W')
                .long_flag("watch")
                .about("Add the current working directory as a repository to watch.")
        )
        .subcommand(
            App::new("unwatch")
                .short_flag('U')
                .long_flag("unwatch")
                .about("Missing description")
        )
        .subcommand(
            App::new("kill")
                .short_flag('K')
                .long_flag("kill")
                .about("Stop the running worker (should only be a single worker).")
        )
        .get_matches();

        match matches.subcommand() {
            Some(("capture", _)) => {
                if let Some(oid) = snapshots::capture(&dir).unwrap() {
                    println!("{}", oid);
                }
            }
            Some(("serve", _)) => {
                let env_filter =
                    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
                Registry::default()
                    .with(env_filter)
                    .with(NestedJsonLayer::new(std::io::stdout))
                    .init();
                tracing::info!(pid = std::process::id());
                poller::start().await;
            }
            Some(("watch", _)) => {
                watch_dir(&dir);
            }
            Some(("unwatch", _)) => unwatch_dir(&dir),
            Some(("kill", _)) => {
                kill();
            }
            _ => unreachable!(),
        }
}

fn watch_dir(path: &std::path::Path) {
    let mut config = Config::load();
    config.set_watch(path.to_str().unwrap().to_string(), WatchConfig::new());
    config.save();
}

fn unwatch_dir(path: &std::path::Path) {
    let mut config = Config::load();
    config.set_unwatch(path.to_str().unwrap().to_string());
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
