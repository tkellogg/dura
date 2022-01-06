use std::process;

use dura::config::{Config, WatchConfig};
use dura::snapshots;
use dura::poller;

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
        Some("unwatch") => {
            unwatch_dir(&dir);
        }
        Some("kill") => {
            kill();
        }
        Some(_) if find_sub_command() => {
            // unreachable. Process already exited.
        }
        _ => {
            eprintln!("dura backs up your work automatically via Git commits

Usage: dura SUBCOMMAND

serve
    Starts the worker that listens for file changes. If another 
    process is aleady running, this will do it's best to terminate
    the other process.

watch
    Add the current working directory as a repository to watch.

kill
    Stop the running worker (should only be a single worker).

capture
    Run a single backup of an entire repository. This is the one
    single iteration of the `serve` control loop.
");
            process::exit(1);
        }
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

/// Look for an executable on the $PATH called `dura-{cmd}`. This
/// enables extending dura by placing shell scripts in, e.g., /usr/local/bin 
///
/// This always either exits `false` or terminates the process. All output,
/// both stdout and stderr, are piped through the current process and the 
/// exit code is also propagated.
///
/// All additional arguments will also be passed to the child process.
fn find_sub_command() -> bool {
    let args: Vec<String> = std::env::args().collect();
    let cmd = match args.get(1) {
        Some(sub) => format!("dura-{}", sub),
        None => { return false },
    };

    let cmd_args = args[2..].as_ref();
    let child_proc = process::Command::new(cmd.as_str())
        .args(cmd_args)
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .spawn();

    match child_proc {
        Ok(mut child) => {
            match child.wait() {
                Ok(status) => {
                    // From docs: On Unix, this will return None if the process was 
                    // terminated by a signal.
                    let code = status.code().unwrap_or(1);
                    process::exit(code)
                }
                Err(_) => false,
            }
        }
        Err(_) => false,
    }
}

