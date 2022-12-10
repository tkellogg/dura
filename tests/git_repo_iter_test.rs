mod util;

mod cached_fs_test {
    use crate::{repo_and_file, set_log_lvl, util};
    use dura::git_repo_iter::{CachedDirIter, CachedFs};
    use std::collections::HashSet;
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::time::Instant;
    use tracing::debug;

    fn iter_to_set(iter: CachedDirIter) -> HashSet<String> {
        iter.map(|dir| {
            dir.components()
                .last()
                .unwrap()
                .as_os_str()
                .to_os_string()
                .into_string()
                .unwrap()
        })
        .collect()
    }

    #[test]
    fn finds_all_dirs_on_startup() {
        let tmp = tempfile::tempdir().unwrap();
        repo_and_file!(tmp, &["repo1"], "foo.txt");
        repo_and_file!(tmp, &["repo2"], "foo.txt");
        let fs = CachedFs::new(Duration::from_millis(50), Duration::from_millis(5));
        assert_eq!(
            iter_to_set(fs.list_dir(tmp.path().to_path_buf())),
            HashSet::from(["repo1".to_string(), "repo2".to_string()])
        );
        // same, but again
        assert_eq!(
            iter_to_set(fs.list_dir(tmp.path().to_path_buf())),
            HashSet::from(["repo1".to_string(), "repo2".to_string()])
        );
    }

    #[test]
    fn finds_nested_dirs() {
        // set_log_lvl!(filter::LevelFilter::TRACE);
        let tmp = tempfile::tempdir().unwrap();
        repo_and_file!(tmp, &["foo", "repo1"], "foo.txt");
        repo_and_file!(tmp, &["foo", "bar", "repo2"], "foo.txt");
        let fs = CachedFs::new(Duration::from_millis(50), Duration::from_millis(5));
        assert_eq!(
            iter_to_set(fs.list_dir(tmp.path().join("foo"))),
            HashSet::from(["repo1".to_string(), "bar".to_string()])
        );
        assert_eq!(
            // again
            iter_to_set(fs.list_dir(tmp.path().join("foo"))),
            HashSet::from(["repo1".to_string(), "bar".to_string()])
        );
        assert_eq!(
            iter_to_set(fs.list_dir(tmp.path().join("foo").join("bar"))),
            HashSet::from(["repo2".to_string()])
        );
        assert_eq!(
            // again
            iter_to_set(fs.list_dir(tmp.path().join("foo").join("bar"))),
            HashSet::from(["repo2".to_string()])
        );
    }

    /// Used for the next couple tests
    fn do_test() -> (usize, HashSet<String>, TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        repo_and_file!(tmp, &["repo1"], "foo.txt");
        repo_and_file!(tmp, &["repo2"], "foo.txt");
        let mut fs = CachedFs::new(Duration::from_millis(50), Duration::from_millis(5));
        let mut found = iter_to_set(fs.list_dir(tmp.path().to_path_buf()));
        assert_eq!(found.len(), 2);
        assert_eq!(
            found,
            HashSet::from(["repo1".to_string(), "repo2".to_string()])
        );

        let mut ret = 0;

        // add new repo and wait until max time
        repo_and_file!(tmp, &["repo3"], "foo.txt");
        let start = Instant::now();
        fs.reset_ttl();
        loop {
            found = iter_to_set(fs.list_dir(tmp.path().to_path_buf()));

            let elapsed = Instant::now() - start;
            if elapsed > Duration::from_millis(50) {
                debug!("exit test loop, timeout");
                break;
            } else if found.len() == 3 {
                debug!("exit test loop, found all");
                dbg!(&found);
                break;
            } else {
                thread::sleep(Duration::from_millis(5));
            }

            fs.cycle();
            ret += 1;
        }

        (ret, found, tmp)
    }

    #[test]
    #[ignore]
    fn finds_new_dirs_within_max_duration() {
        set_log_lvl!(filter::LevelFilter::TRACE);
        let (loop_break, found, _tmp) = do_test();

        assert_eq!(
            found,
            HashSet::from([
                "repo1".to_string(),
                "repo2".to_string(),
                "repo3".to_string(),
            ])
        );

        // this assertion may be flaky. just re-run
        assert!(loop_break > 0);
    }

    /// Run the previous test a large number of times and test how often
    /// the cache gets invalidated early
    #[test]
    fn invalidates_cache_roughly_along_normal_distribution() {
        // set_log_lvl!(filter::LevelFilter::TRACE);

        let result = (0..100).map(|_| do_test()).collect::<Vec<_>>();
        let exit_loop = result.iter().map(|x| x.0).collect::<Vec<_>>();
        dbg!(&exit_loop);

        // the tails are non-zero
        assert!(*exit_loop.iter().min().unwrap() <= 2);
        assert!(*exit_loop.iter().max().unwrap() >= 8);

        // it usually lands in the middle
        let mid_range = exit_loop.iter().filter(|x| 3 <= **x && **x <= 7).count();
        // this assert might be flaky, just rerun it
        assert!(mid_range > 20, "failed: 3 <= {} <= 7", mid_range);
    }
}
