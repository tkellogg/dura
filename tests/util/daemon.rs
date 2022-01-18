use std::io::{BufRead, BufReader};
use std::process::{Child, ChildStdout};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Main-thread side of a process watcher. The process that's launched is exposed as messages
/// (per-line) over a mpsc channel. This is intended to simplify, speed up, and generally make the
/// tests more reliable when they dispatch asynchronously to `dura serve`. However, nothing abot
/// this is intended to be specific to dura.
pub struct Daemon {
    mailbox: Receiver<Option<String>>,
    pub child: Child,
    /// Signals to kill daemon thread if this goes <= 0, like a CountDownLatch
    kill_sign: Arc<Mutex<i32>>,
}

impl Daemon {
    pub fn new(mut child: Child) -> Self {
        let kill_sign = Arc::new(Mutex::new(1));
        Self {
            mailbox: Self::attach(
                child
                    .stdout
                    .take()
                    .expect("Configure Command to capture stdout"),
                Arc::clone(&kill_sign),
            ),
            child,
            kill_sign,
        }
    }

    /// Spawn another thread to watch the child process. It attaches to stdout and sends each line
    /// over the channel. It sends a None right before it quits, either due to an error or EOF.
    fn attach(stdout: ChildStdout, kill_sign: Arc<Mutex<i32>>) -> Receiver<Option<String>> {
        let (sender, receiver) = channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                {
                    // check to see if the daemon is killed
                    if *kill_sign.lock().unwrap() <= 0 {
                        break;
                    }
                }
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => {
                        sender.send(None).unwrap();
                        break;
                    }
                    Ok(_) => {
                        sender.send(Some(line)).unwrap();
                    }
                    Err(e) => {
                        eprintln!("Error in daemon: {:?}", e);
                        sender.send(None).unwrap();
                        break;
                    }
                }
            }
        });
        receiver
    }

    /// Read a line from the child process, waiting at most timeout_secs.
    pub fn read_line(&self, timeout_secs: u64) -> Option<String> {
        self.mailbox
            .recv_timeout(Duration::from_secs(timeout_secs))
            .unwrap()
    }

    pub fn kill(&mut self) {
        let mut kill_sign = self.kill_sign.lock().unwrap();
        *kill_sign -= 1;
        self.child.kill().unwrap();
    }
}
