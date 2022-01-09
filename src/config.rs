use std::collections::HashMap;
use std::fs::{create_dir_all, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::{env, io};

use serde::{Deserialize, Serialize};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub pid: Option<u32>,
    pub repos: HashMap<String, WatchConfig>,
}

impl Config {
    pub fn empty() -> Self {
        Self {
            pid: None,
            repos: HashMap::new(),
        }
    }

    pub fn default_path() -> PathBuf {
        Self::get_dura_home().join("config.json")
    }

    /// Location of all config & database files. By default this is ~/.config/dura but can be
    /// overridden by setting DURA_HOME environment variable.
    fn get_dura_home() -> PathBuf {
        // The environment variable lets us run tests independently, but I'm sure someone will come
        // up with another reason to use it.
        if let Ok(env_var) = env::var("DURA_HOME") {
            if !env_var.is_empty() {
                return env_var.into();
            }
        }

        home::home_dir()
        .expect("Could not find your home directory. The default is ~/.config/dura but it can also \
                be controlled by setting the DURA_HOME environment variable.")
        .join(".config/dura")
    }

    /// Load Config from default path
    pub fn load() -> Self {
        Self::load_file(Self::default_path().as_path()).unwrap_or_else(|_| Self::empty())
    }

    pub fn load_file(path: &Path) -> Result<Self> {
        let reader = io::BufReader::new(File::open(path)?);
        let res = serde_json::from_reader(reader)?;
        Ok(res)
    }

    /// Save config to disk in ~/.config/dura/config.json
    pub fn save(&self) {
        self.save_to_path(Self::default_path().as_path())
    }

    pub fn create_dir(path: &Path) {
        path.parent().map(|dir| create_dir_all(dir).unwrap());
    }

    /// Used by tests to save to a temp dir
    pub fn save_to_path(&self, path: &Path) {
        Self::create_dir(path);

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .unwrap();

        let writer = io::BufWriter::new(file);
        serde_json::to_writer(writer, self).unwrap();
    }

    pub fn set_watch(&mut self, path: String, cfg: WatchConfig) {
        if self.repos.contains_key(&path) {
            println!("{} is already being watched", path)
        } else {
            self.repos.insert(path.clone(), cfg);
            println!("Started watching {}", path)
        }
    }

    pub fn set_unwatch(&mut self, path: String) {
        match self.repos.remove(&path) {
            Some(_) => println!("Stopped watching {}", path),
            None => println!("{} is not being watched", path),
        }
    }
}