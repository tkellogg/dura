use git2::{BranchType, Commit, Repository};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::ops::Add;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::Result;
use walkdir::{DirEntry, WalkDir};

/// OPTIMIZATION for checking for changes
///
/// Provides a function, dir_changed, that is a much faster way to detect if any files in
/// a repository have changed, vs the naive method of trying to commit the repo. This peeks at
/// the file timestamp, which is typically cached in memory. The previous way to do it was to
/// let Git2 make a commit, which triggered a whole lot of I/O and hashing.
pub struct PollGuard {
    git_cache: HashMap<PathBuf, Repository>,
}

impl PollGuard {
    pub fn new() -> Self {
        Self {
            git_cache: Default::default(),
        }
    }

    pub fn dir_changed(&mut self, dir: &Path) -> bool {
        let watermark = match self.get_watermark(dir) {
            Ok(watermark) => watermark,
            // True because we want to turn off this optimization
            Err(_) => return true,
        };

        fn compare_times(modified: SystemTime, watermark: SystemTime) -> Result<bool> {
            let duration = modified.duration_since(watermark)?;
            Ok(duration.as_secs_f32() > 1.0)
        }

        fn get_file_time(entry: walkdir::Result<DirEntry>) -> Result<SystemTime> {
            Ok(entry?.metadata()?.modified()?)
        }

        for entry in WalkDir::new(dir) {
            if let Ok(modified) = get_file_time(entry) {
                if compare_times(modified, watermark).unwrap_or(false) {
                    return true;
                }
            }
        }
        false
    }

    /// Find the last known commit timestamp
    fn get_watermark(&mut self, path: &Path) -> Result<SystemTime> {
        // Get git repo, create if necessary
        let repo: &Repository = match self.git_cache.entry(path.into()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let new = Repository::open(path)?;
                entry.insert(new)
            }
        };

        fn get_time(commit: &Commit) -> SystemTime {
            SystemTime::UNIX_EPOCH.add(Duration::from_secs(commit.time().seconds() as u64))
        }

        fn get_dura_time(head: &Commit, repo: &Repository) -> Result<SystemTime> {
            let branch_name = format!("dura/{}", head.id());
            let ret = repo
                .find_branch(&branch_name, BranchType::Local)?
                .get()
                .peel_to_commit()?;
            Ok(get_time(&ret))
        }

        // get commit time and fallback to time of HEAD
        let head = repo.head()?.peel_to_commit()?;
        Ok(get_dura_time(&head, repo).unwrap_or_else(|_| get_time(&head)))
    }
}

/// Implemented manually because Repository doesn't implement it
impl Debug for PollGuard {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("PollGuard { ")?;
        for dir in self.git_cache.keys() {
            f.write_str(dir.to_str().unwrap_or("n/a"))?;
            f.write_str(", ")?;
        }
        f.write_str(" }")?;
        Ok(())
    }
}

impl Default for PollGuard {
    fn default() -> Self {
        Self::new()
    }
}
