use std::{
    ops, path,
    process::{Child, Command},
    thread, time,
};

use dura::config::Config;

/// Utility to start dura asynchronously (e.g. dura serve) and kill the process when this goes out
/// of scope. This helps us do end-to-end tests where we invoke the executable, possibly multiple
/// different processes.
pub struct Dura {
    primary: Option<Child>,
    secondary: Option<Child>,
    home_dir: tempfile::TempDir,
}

impl Dura {
    pub fn new() -> Self {
        Self {
            primary: None,
            secondary: None,
            home_dir: tempfile::tempdir().unwrap(),
        }
    }

    pub fn start_async(&mut self, args: &[&str], is_primary: bool) {
        println!("$ dura {} &", args.join(" "));
        let exe = env!("CARGO_BIN_EXE_dura").to_string();
        let child = Command::new(exe)
            .args(args)
            .env("DURA_HOME", self.home_dir.path())
            .spawn()
            .unwrap();

        if is_primary {
            self.primary = Some(child);
        } else {
            self.secondary = Some(child);
        }
    }

    pub fn run(&self, args: &[&str]) {
        println!("$ dura {}", args.join(" "));
        let mut child = Command::new("target/debug/dura")
            .args(args)
            .env("DURA_HOME", self.home_dir.path())
            .spawn()
            .unwrap();

        let _ = child.wait().unwrap();
    }

    pub fn pid(&self, is_primary: bool) -> Option<u32> {
        if is_primary {
            self.primary.as_ref().map(|ps| ps.id())
        } else {
            self.secondary.as_ref().map(|ps| ps.id())
        }
    }

    pub fn config_path(&self) -> path::PathBuf {
        self.home_dir.path().join("config.json")
    }

    pub fn get_config(&self) -> Option<Config> {
        println!("$ cat ~/.config/dura/config.json");
        let cfg = Config::load_file(self.config_path().as_path()).ok();
        println!("{:?}", cfg);
        cfg
    }

    pub fn save_config(&self, cfg: &Config) {
        cfg.save_to_path(self.config_path().as_path());
    }

    pub fn wait(&self) {
        // This hack isn't going to work. Another idea is to read lines
        // from stdout as a signal to proceed.
        thread::sleep(time::Duration::from_secs(6));
    }
}

impl ops::Drop for Dura {
    /// Force Kill. Not the "kind kill" that is `dura kill`
    ///
    /// Ensure the process is stopped. Each test gets a unique config file path, so processes
    /// should stay independent and isolated as long as no one is running `ps aux`
    fn drop(&mut self) {
        // don't handle kill errors.
        let _ = self.primary.as_mut().map(|ps| ps.kill());
        let _ = self.secondary.as_mut().map(|ps| ps.kill());
    }
}
