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
}
