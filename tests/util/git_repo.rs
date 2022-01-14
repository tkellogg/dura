use std::{fs, path, process::Command};
use git2::Repository;

/// A test utility to make our tests more readable
pub struct GitRepo {
    // implements Drop to delete the directory
    pub dir: path::PathBuf,

    // Source of entropy for change_file
    counter: u32,
}

impl GitRepo {
    pub fn new(dir: path::PathBuf) -> Self {
        Self { dir, counter: 0 }
    }

    pub fn repo(&self) -> Repository {
        Repository::open(self.dir.as_path()).unwrap()
    }

    pub fn git(&self, args: &[&str]) -> Option<String> {
        println!("$ git {}", args.join(" "));
        let git_dir = self.dir.as_path().join(path::Path::new(".git"));

        let child_proc = Command::new("git")
            .args(
                [
                    &[
                        "--git-dir",
                        git_dir.to_str().unwrap(),
                        "--work-tree",
                        self.dir.as_path().to_str().unwrap(),
                    ],
                    args,
                ]
                .concat(),
            )
            .output();

        if let Ok(output) = child_proc {
            let text = String::from_utf8(output.stdout).unwrap();
            if !text.is_empty() {
                println!("{}", text);
            }
            let err = String::from_utf8(output.stderr).unwrap();
            if !err.is_empty() {
                println!("{}", err);
            }
            if !output.status.success() {
                // This cleans up test development by causing us to fail earlier
                None
            } else {
                Some(text)
            }
        } else {
            None
        }
    }

    pub fn init(&self) {
        fs::create_dir_all(self.dir.as_path()).unwrap();
        let _ = self.git(&["init"]).unwrap();
        let _ = self.git(&["--version"]).unwrap();
        let _ = self.git(&["checkout", "-b", "master"]).unwrap();
        // Linux & Windows will fail on `git commit` if these aren't set
        let _ = self.git(&["config", "user.name", "duratest"]).unwrap();
        let _ = self.git(&["config", "user.email", "duratest@dura.io"]).unwrap();
    }

    pub fn commit_all(&self) {
        self.git(&["add", "."]).unwrap();
        self.git(&["status"]).unwrap();
        self.git(&["commit", "-m", "test"]).unwrap();
    }

    pub fn write_file(&self, path: &str) {
        let content = "initial rev";
        let path_obj = self.dir.as_path().join(path);
        println!("$ echo '{}' > {}", content, path);
        fs::write(path_obj, content).unwrap();
    }

    /// Every time this is called it overwrites the file with **different** contents.
    pub fn change_file(&mut self, path: &str) {
        self.counter += 1;
        let content = format!("change {}", self.counter);
        println!("$ echo '{}' > {}", content, path);
        let path_obj = self.dir.as_path().join(path);
        fs::write(path_obj, content).unwrap();
    }

    pub fn set_config(&self, name: &str, value: &str) {
        self.git(&["config", name, value]).unwrap();
    }
}
