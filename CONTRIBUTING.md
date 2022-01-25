# Contributing to `dura`

# Pull request process
1. Discuss changes before starting. This helps avoid awkward situations, like where something has already been tried or isn't feasible for a non-obvious reason.
2. Add tests, if possible
    * [`startup_test.rs`](https://github.com/tkellogg/dura/blob/master/tests/startup_test.rs) is a good place to test out new functionality, and the test code reads fairly well.
    * Unit tests are preferred, when feasible. They go inside source files.
3. Run `$ ./scripts/pre-commit.sh` before pushing. This does almost everything that happens in CI, just faster.
4. Explain the behavior as best as possible. Things like screenshots and GIFs can be helpful when it's visual.
5. Breathe deep. Smell the fresh clean air.

We try to get to PRs within a day. We're usually quicker than that, but sometimes things slide through the cracks.

Oh! And please be kind. We're all here because we want to help other people. Please remember that.


# Coding guidelines

## Printing output
* All `stdout` is routed through the logger and is JSON.
* Messages to the user should be on `stderr` and are plain text (e.g. can't take a lock)
* Use serialized structs to write JSON logs, so that the structure remains mostly backward compatible. Try not to rename fields, in case someone has written scripts against it.


## Unit tests vs Integration tests
For the purposes of this project, "integration tests" use the filesystem. The [official Rust recommendation](https://doc.rust-lang.org/book/ch11-03-test-organization.html) 
is:

* **Unit tests** go inline inside source files, in a `#[cfg(test)]` module. Structure your code so that
  you can use these to test private functions without using the external dependencies like the 
  filesystem.
* **Integration tests** go "externally", in the `/tests` folder. Use the utilities in `tests/util` to 
  work with external dependencies easier.
  * `git_repo` — makes it easy to work with Git repositories in a temp directory. It does it in a way
    that tests can continue to run in parallel without interfering with each other.
  * `dura` — makes it easy to call the real `dura` executable in a sub-process. This makes it 
    possible to run tests in parallel by setting environment varibales only for the sub-process 
    (e.g. `$DURA_HOME`). It also uses the `util::daemon` module to facilitate working with `dura serve`
    by allowing you to make a blocking call to `read_line` to wait the minimum amount of time for
    an activity to happen (like startup or snapshots).


