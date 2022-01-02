use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs::{File, create_dir_all};
use std::io;

use serde::{Serialize, Deserialize};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize, Deserialize)]
pub struct WatchConfig {
}

impl WatchConfig {
    pub fn new() -> Self {
        Self {
        }
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

    fn default_path() -> PathBuf {
        let mut home = std::env::home_dir().unwrap();
        home.push(".config");
        home.push("duralumin");
        home.push("config.json");
        home
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
        let file = match path.as_path().exists() {
            true => File::open(path.as_path()).unwrap(),
            false => {
                create_dir_all(path.as_path().parent().unwrap()).unwrap();
                File::create(path.as_path()).unwrap()
            },
        };

        let writer = io::BufWriter::new(file);
        serde_json::to_writer(writer, self).unwrap()
    }

    pub fn set_watch(&mut self, path: String, cfg: WatchConfig) {
        self.repos.insert(path, cfg);
    }
}

