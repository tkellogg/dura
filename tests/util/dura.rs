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
        let exe = env!("CARGO_BIN_EXE_dura").to_string();
        let child_proc = Command::new(exe)
            .args(args)
            .env("DURA_HOME", self.home_dir.path())
            .output();

        if let Ok(output) = child_proc {
            if !output.status.success() {
                // This cleans up test development by causing us to fail earlier
                return;
            }
            let text = String::from_utf8(output.stdout).unwrap();
            if !text.is_empty() {
                println!("{}", text);
            }
            let err = String::from_utf8(output.stderr).unwrap();
            if !err.is_empty() {
                println!("{}", err);
            }
        }
    }

    pub fn run_in_dir(&self, args: &[&str], dir: &path::Path) {
        println!("$ dura {}", args.join(" "));
        let exe = env!("CARGO_BIN_EXE_dura").to_string();
        let child_proc = Command::new(exe)
            .args(args)
            .env("DURA_HOME", self.home_dir.path())
            .current_dir(dir)
            .output();

        if let Ok(output) = child_proc {
            if !output.status.success() {
                // This cleans up test development by causing us to fail earlier
                return;
            }
            let text = String::from_utf8(output.stdout).unwrap();
            if !text.is_empty() {
                println!("{}", text);
            }
            let err = String::from_utf8(output.stderr).unwrap();
            if !err.is_empty() {
                println!("{}", err);
            }
        }
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

    pub fn git_repos(&self) -> HashSet<path::PathBuf> {
        match self.get_config() {
            Some(cfg) => cfg.git_repos().collect(),
            None => HashSet::new(),
        }
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
