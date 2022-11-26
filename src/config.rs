use std::collections::BTreeMap;
use std::fs::{create_dir_all, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::{env, fs};

use serde::{Deserialize, Serialize};

use crate::git_repo_iter::GitRepoIter;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct WatchConfig {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub max_depth: u8,
}

impl WatchConfig {
    pub fn new() -> Self {
        Self {
            include: vec![],
            exclude: vec![],
            max_depth: 255,
        }
    }
}

impl Default for WatchConfig {
    fn default() -> Self {
        WatchConfig::new()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    // When commit_exclude_git_config is true,
    // never use any git configuration to sign dura's commits.
    // Defaults to false
    #[serde(default)]
    pub commit_exclude_git_config: bool,
    pub commit_author: Option<String>,
    pub commit_email: Option<String>,
    pub repos: BTreeMap<String, Rc<WatchConfig>>,
}

impl Config {
    pub fn empty() -> Self {
        Self {
            commit_exclude_git_config: false,
            commit_author: None,
            commit_email: None,
            repos: BTreeMap::new(),
        }
    }

    pub fn default_path() -> PathBuf {
        Self::get_dura_config_home().join("config.toml")
    }

    /// Location of all config. By default
    ///
    /// Linux   :   $XDG_CONFIG_HOME/dura or $HOME/.config/dura
    /// macOS   :   $HOME/Library/Application Support
    /// Windows :   %AppData%\Roaming\dura
    ///
    /// This can be overridden by setting DURA_CONFIG_HOME environment variable.
    fn get_dura_config_home() -> PathBuf {
        // The environment variable lets us run tests independently, but I'm sure someone will come
        // up with another reason to use it.
        if let Ok(env_var) = env::var("DURA_CONFIG_HOME") {
            if !env_var.is_empty() {
                return env_var.into();
            }
        }

        dirs::config_dir()
            .expect("Could not find your config directory. The default is ~/.config/dura but it can also \
                be controlled by setting the DURA_CONFIG_HOME environment variable.")
            .join("dura")
    }

    /// Load Config from default path
    pub fn load() -> Self {
        Self::load_file(Self::default_path().as_path()).unwrap_or_else(|_| Self::empty())
    }

    pub fn load_file(path: &Path) -> Result<Self> {
        let mut reader = BufReader::new(File::open(path)?);

        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;

        let res = toml::from_slice(buffer.as_slice())?;
        Ok(res)
    }

    /// Save config to disk in ~/.config/dura/config.toml
    pub fn save(&self) {
        self.save_to_path(Self::default_path().as_path())
    }

    pub fn create_dir(path: &Path) {
        if let Some(dir) = path.parent() {
            create_dir_all(dir)
                .unwrap_or_else(|_| panic!("Failed to create directory at `{}`.\
                    Dura stores its configuration in `{}/config.toml`, \
                    where you can instruct dura to watch patterns of Git repositories, among other things. \
                    See https://github.com/tkellogg/dura for more information.", dir.display(), path.display()))
        }
    }

    /// Attempts to create parent dirs, serialize `self` as TOML and write to disk.
    pub fn save_to_path(&self, path: &Path) {
        Self::create_dir(path);

        let config_string = match toml::to_string(self) {
            Ok(v) => v,
            Err(e) => {
                println!("Unexpected error when deserializing config: {}", e);
                return;
            }
        };

        match fs::write(path, config_string) {
            Ok(_) => (),
            Err(e) => println!("Unable to initialize dura config file: {}", e),
        }
    }

    pub fn set_watch(&mut self, path: String, cfg: WatchConfig) {
        let abs_path = fs::canonicalize(path).expect("The provided path is not a directory");
        let abs_path = abs_path
            .to_str()
            .expect("The provided path is not valid unicode");

        if self.repos.contains_key(abs_path) {
            println!("{} is already being watched", abs_path)
        } else {
            self.repos.insert(abs_path.to_string(), Rc::new(cfg));
            println!("Started watching {}", abs_path)
        }
    }

    pub fn set_unwatch(&mut self, path: String) {
        let abs_path = fs::canonicalize(path).expect("The provided path is not a directory");
        let abs_path = abs_path
            .to_str()
            .expect("The provided path is not valid unicode")
            .to_string();

        match self.repos.remove(&abs_path) {
            Some(_) => {
                println!("Stopped watching {}", abs_path);
            }
            None => println!("{} is not being watched", abs_path),
        }
    }

    pub fn git_repos(&self) -> GitRepoIter {
        GitRepoIter::new(self)
    }
}
