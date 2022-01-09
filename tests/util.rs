use std::{fs, path, ops, thread, time, env};
use std::path::Path;
use std::process::{Command, Child};

use tempfile;

use dura::config::Config;

/// A test utility to make our tests more readable
pub struct GitRepo {
    // implements Drop to delete the directory
    pub dir: tempfile::TempDir,

    // Source of entropy for change_file
    counter: u32,
}

impl GitRepo {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        Self { dir, counter: 0 }
    }

    pub fn new_in<P: AsRef<Path>>(dir: P) -> Self {
        let dir = tempfile::tempdir_in(dir).unwrap();
        Self { dir, counter: 0 }
    }

    pub fn git(&self, args: &[&str]) -> Option<String> {
        println!("$ git {}", args.join(" "));
        let git_dir = self.dir.path().join(path::Path::new(".git"));

        let child_proc = Command::new("git")
            .args([&["--git-dir", git_dir.to_str().unwrap(), "--work-tree", self.dir.path().to_str().unwrap()], args].concat())
            .output();

        if let Ok(output) = child_proc {
            if !output.status.success() {
                // This cleans up test development by causing us to fail earlier
                return None
            }
            let text = String::from_utf8(output.stdout).unwrap();
            if text.len() > 0 {
                println!("{}", text);
            }
            let err = String::from_utf8(output.stderr).unwrap();
            if err.len() > 0 {
                println!("{}", err);
            }
            Some(text)
        } else {
            None
        }
    }

    pub fn init(&self) {
        let _ = self.git(&["init"]);
        let _ = self.git(&["checkout", "-b", "master"]);
    }

    pub fn commit_all(&self) {
        self.git(&["add", "."]);
        self.git(&["status"]);
        self.git(&["commit", "-m", "test"]);
    }

    pub fn write_file(&self, path: &str) {
        let content = "initial rev";
        let path_obj = self.dir.path().join(path);
        println!("$ echo '{}' > {}", content, path);
        fs::write(path_obj, content).unwrap();
    }

    /// Every time this is called it overwrites the file with **different** contents.
    pub fn change_file(&mut self, path: &str) {
        self.counter += 1;
        let content = format!("change {}", self.counter);
        println!("$ echo '{}' > {}", content, path);
        let path_obj = self.dir.path().join(path);
        fs::write(path_obj, content).unwrap();
    }
}

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


