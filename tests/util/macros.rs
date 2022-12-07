
/// Create a Git repo with a single file and commit
#[macro_export]
macro_rules! repo_and_file {
    ( $tmp:expr, $file_name:expr ) => {{
        let repo = util::git_repo::GitRepo::new($tmp.path().to_path_buf());
        repo.init();
        repo.write_file($file_name);
        repo.commit_all();
        repo
    }};
    ( $tmp:expr, $path:expr, $file_name:expr ) => {{
        let mut path_buf = $tmp.path().to_path_buf();
        for p in $path {
            path_buf.push(p);
        }
        let repo = util::git_repo::GitRepo::new(path_buf.clone());
        repo.init();
        repo.write_file($file_name);
        repo.commit_all();
        repo
    }};
}

#[macro_export]
macro_rules! set_log_lvl {
    ($lvl:expr) => {{
        use tracing_subscriber::{filter, fmt, reload, prelude::*};

        let (filter, _reload_handle) = reload::Layer::new($lvl);
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::Layer::default())
            .init();
    }};
}
