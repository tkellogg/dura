use std::collections::HashMap;
use std::fs::{create_dir_all, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize, Deserialize)]
pub struct WatchConfig {}

impl WatchConfig {
    pub fn new() -> Self {
        Self {}
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub pid: Option<u32>,
    pub repos: HashMap<String, WatchConfig>,
}

impl Config {
    fn empty() -> Self {
        Self {
            pid: None,
            repos: HashMap::new(),
        }
    }

    pub fn default_path() -> PathBuf {
        std::env::home_dir().unwrap().join(".config/dura/config.json")
    }

    /// Load Config from default path
    pub fn load() -> Self {
        match Self::load_file(Self::default_path().as_path()) {
            Ok(obj) => obj,
            Err(_) => Self::empty(),
        }
    }

    fn load_file(path: &Path) -> Result<Self> {
        let reader = io::BufReader::new(File::open(path)?);
        let res = serde_json::from_reader(reader)?;
        Ok(res)
    }

    pub fn save(&self) {
        let path = Self::default_path();
        path.clone().parent().map(create_dir_all);

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(path.as_path())
            .unwrap();

        let writer = io::BufWriter::new(file);
        serde_json::to_writer(writer, self).unwrap();
        println!("Wrote {}", path.to_str().unwrap());
    }

    pub fn set_watch(&mut self, path: String, cfg: WatchConfig) {
        self.repos.insert(path, cfg);
    }
}
