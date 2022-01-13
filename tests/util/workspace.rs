mod util;

/// Test utility to set up test dura environment
pub struct Workspace {
    pub dir: path::PathBuf,
    pub config_dir
}

impl Workspace {
    pub fn new(dir: path::PathBuf) -> Self {
        Self { dir }
    }

    pub fn init_dura(dir: path::PathBuf) -> Self {

    }
}
