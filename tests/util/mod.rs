// Not every test module -- which are compiled at separate binaries, use every function, causing dead_code to be emitted.
#![allow(dead_code)]

pub mod daemon;
pub mod dura;
pub mod git_repo;
