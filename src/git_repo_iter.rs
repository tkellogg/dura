use std::collections::btree_map;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::config::{Config, WatchConfig};
use crate::snapshots;

/// Internal structure to facilitate "recursion" without blowing up the stack. Without this, we
/// could call self.next() recursively whenever there was an I/O error or when we reached the end
/// of a directory listing. There's no stack space used because we just mutate GitRepoIter, so
/// might as well turn it into a loop.
enum CallState {
    Yield(PathBuf),
    Recurse,
    Done,
}

/// Iterator over all Git repos covered by a config.
///
/// The process is naturally recursive, traversing a directory structure, which made it a poor fit
/// for a more typical filter/map chain.
///
/// Function recursion is used in a few cases:
///  1. Errors: If we get an I/O error, we'll call self.next() again
///  2. Empty iterator: If we get to the end of a sub-iterator, pop & start from the top
///
pub struct GitRepoIter<'a> {
    config_iter: btree_map::Iter<'a, String, Rc<WatchConfig>>,
    /// A stack, because we can't use recursion with an iterator (at least not between elements)
    sub_iter: Vec<(Rc<PathBuf>, Rc<WatchConfig>, fs::ReadDir)>,
}

impl<'a> GitRepoIter<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self {
            config_iter: config.repos.iter(),
            sub_iter: Vec::new(),
        }
    }

    fn get_next(&mut self) -> CallState {
        // pop
        //
        // Use pop here to manage the lifetime of the iterator. If we used last/peek, we would
        // borrow a shared reference, which precludes us from borrowing as mutable when we want to
        // use the iterator. But that means we have to return it to the vec.
        match self.sub_iter.pop() {
            Some((base_path, watch_config, mut dir_iter)) => {
                let mut next_next: Option<(Rc<PathBuf>, Rc<WatchConfig>, fs::ReadDir)> = None;
                let mut ret_val = CallState::Recurse;
                let max_depth: usize = watch_config.max_depth.into();
                if let Some(Ok(entry)) = dir_iter.next() {
                    let child_path = entry.path();
                    if is_valid_directory(base_path.as_path(), child_path.as_path(), &watch_config)
                    {
                        if snapshots::is_repo(child_path.as_path()) {
                            ret_val = CallState::Yield(child_path);
                        } else if self.sub_iter.len() < max_depth {
                            if let Ok(child_dir_iter) = fs::read_dir(child_path.as_path()) {
                                next_next = Some((
                                    Rc::clone(&base_path),
                                    Rc::clone(&watch_config),
                                    child_dir_iter,
                                ))
                            }
                        }
                    }
                    // un-pop
                    self.sub_iter
                        .push((Rc::clone(&base_path), Rc::clone(&watch_config), dir_iter));
                }
                if let Some(tuple) = next_next {
                    // directory recursion
                    self.sub_iter.push(tuple);
                }
                ret_val
            }
            None => {
                // Finished dir, queue up next hashmap pair
                match self.config_iter.next() {
                    Some((base_path, watch_config)) => {
                        let path = PathBuf::from(base_path);
                        let dir_iter_opt = path.parent().and_then(|p| fs::read_dir(p).ok());
                        if let Some(dir_iter) = dir_iter_opt {
                            // clone because we're going from more global to less global scope
                            self.sub_iter
                                .push((Rc::new(path), Rc::clone(&watch_config), dir_iter));
                        }
                        CallState::Recurse
                    }
                    // The end. The real end. This is it.
                    None => CallState::Done,
                }
            }
        }
    }
}

impl<'a> Iterator for GitRepoIter<'a> {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.get_next() {
                CallState::Yield(path) => return Some(path),
                CallState::Recurse => continue,
                CallState::Done => return None,
            }
        }
    }
}

/// Checks the provided `child_path` is a directory.
/// If either `includes` or `excludes` are set,
/// checks whether the path is included/excluded respectively.
fn is_valid_directory(base_path: &Path, child_path: &Path, value: &WatchConfig) -> bool {
    if !child_path.is_dir() {
        return false;
    }

    if !child_path.starts_with(base_path) {
        return false;
    }

    let includes = &value.include;
    let excludes = &value.exclude;

    let mut include = true;

    if !excludes.is_empty() {
        include = !excludes
            .iter()
            .any(|exclude| child_path.starts_with(base_path.join(exclude)));
    }

    if !include && !includes.is_empty() {
        include = includes
            .iter()
            .any(|include| base_path.join(include).starts_with(child_path));
    }

    include
}
