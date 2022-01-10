use std::fs::OpenOptions;
use std::path::Path;

use clap::{arg, App, AppSettings, Arg, Values};
use dura::config::{Config, WatchConfig};
use dura::logger::NestedJsonLayer;
use dura::poller;
use dura::snapshots;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

#[tokio::main]
async fn main() {
    let cwd = std::env::current_dir().unwrap();

    let arg_directory = Arg::new("directory")
        .default_value_os(cwd.as_os_str())
        .help("The directory to watch. Defaults to current directory");

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
                .arg(arg_directory.clone())
        )
        .subcommand(
            App::new("serve")
                .short_flag('S')
                .long_flag("serve")
                .about("Starts the worker that listens for file changes. If another process is already running, this will do it's best to terminate the other process.")
                .arg(
                    arg!(--logfile)
                    .required(false)
                    .help("Sets custom logfile. Default is logging to stdout")
        ))
        .subcommand(
            App::new("watch")
                .short_flag('W')
                .long_flag("watch")
                .about("Add the current working directory as a repository to watch.")
                .arg(arg_directory.clone())
                .arg(arg!(-i --include)
                    .required(false)
                    .takes_value(true)
                    .use_delimiter(true)
                    .require_delimiter(true)
                    .help("Overrides excludes by re-including specific directories relative to the watch directory.")
                )
                .arg(arg!(-e --exclude)
                    .required(false)
                    .takes_value(true)
                    .use_delimiter(true)
                    .require_delimiter(true)
                    .help("Excludes specific directories relative to the watch directory")
                )
                .arg(arg!(-d --maxdepth)
                    .required(false)
                    .default_value("255")
                    .help("Determines the depth to recurse into when scanning directories")
                )
        )
        .subcommand(
            App::new("unwatch")
                .short_flag('U')
                .long_flag("unwatch")
                .about("Missing description")
                .arg(arg_directory)
        )
        .subcommand(
            App::new("kill")
                .short_flag('K')
                .long_flag("kill")
                .about("Stop the running worker (should only be a single worker).")
        )
        .get_matches();

    match matches.subcommand() {
        Some(("capture", m)) => {
            let dir = Path::new(m.value_of("directory").unwrap());
            if let Some(oid) = snapshots::capture(dir).unwrap() {
                println!("{}", oid);
            }
        }
        Some(("serve", arg_matches)) => {
            let env_filter =
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

            match arg_matches.value_of("logfile") {
                Some(logfile) => {
                    let file = logfile.to_string();
                    Registry::default()
                        .with(env_filter)
                        .with(NestedJsonLayer::new(move || {
                            let result_open_file =
                                OpenOptions::new().append(true).create(true).open(&file);
                            match result_open_file {
                                Ok(f) => f,
                                Err(e) => {
                                    eprintln!(
                                        "Unable to open file {} for logging due to {}",
                                        file, e
                                    );
                                    std::process::exit(1);
                                }
                            }
                        }))
                        .init();
                }
                None => {
                    Registry::default()
                        .with(env_filter)
                        .with(NestedJsonLayer::new(std::io::stdout))
                        .init();
                }
            }

            tracing::info!(pid = std::process::id());
            poller::start().await;
        }
        Some(("watch", arg_matches)) => {
            let dir = Path::new(arg_matches.value_of("directory").unwrap());

            let include = arg_matches
                .values_of("include")
                .unwrap_or(Values::default())
                .map(|s| s.to_string())
                .collect::<Vec<String>>();
            let exclude = arg_matches
                .values_of("exclude")
                .unwrap_or(Values::default())
                .map(|s| s.to_string())
                .collect::<Vec<String>>();
            let max_depth = arg_matches
                .value_of("maxdepth")
                .unwrap_or("255")
                .parse::<u8>()
                .expect("Max depth must be between 0-255");

            let watch_config = WatchConfig {
                include,
                exclude,
                max_depth,
            };

            watch_dir(dir, watch_config);
        }
        Some(("unwatch", arg_matches)) => {
            let dir = Path::new(arg_matches.value_of("directory").unwrap());
            unwatch_dir(dir)
        }
        Some(("kill", _)) => {
            kill();
        }
        _ => unreachable!(),
    }
}

fn watch_dir(path: &std::path::Path, watch_config: WatchConfig) {
    let mut config = Config::load();
    config.set_watch(path.to_str().unwrap().to_string(), watch_config);
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
