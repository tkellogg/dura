# Dura

[![Build][build badge]][build action]

Dura is a background process that watches your Git repositories and commits your uncommitted changes without impacting
HEAD, the current branch, or the Git index (staged files). If you ever get into an "oh snap!" situation where you think
you just lost days of work, checkout a `dura` branch and recover.

Without `dura`, you use Ctrl-Z in your editor to get back to a good state. That's so 2021. Computers crash and Ctrl-Z
only works on files independently. Dura snapshots changes across the entire repository as-you-go, so you can revert to
"4 hours ago" instead of "hit Ctrl-Z like 40 times or whatever". Finally, some sanity.

## How to use

Run it in the background:

```bash
$ dura serve &
```

The `serve` can happen in any directory. The `&` is Unix shell syntax to run the process in the background, meaning that you can start
`dura` and then keep using the same terminal window while `dura` keeps running. You could also run `dura serve` in a
window that you keep open.

Let `dura` know which repositories to watch:

```bash
$ cd some/git/repo
$ dura watch
```

Right now, you have to `cd` into each repo that you want to watch, one at a time.

If you have thoughts on how to do this better, share them [here](https://github.com/tkellogg/dura/issues/3). Until that's sorted, you can
run something like `find ~ -type d -name .git -prune | xargs -I= sh -c "cd =/..; dura watch"` to get started on your existing repos.

Make some changes. No need to commit or even stage them. Use any Git tool to see the `dura` branches:

```bash
$ git log --all
```

`dura` produces a branch for every real commit you make and makes commits to that branch without impacting your working
copy. You keep using Git exactly as you did before.


Let `dura` know that it should stop running in the background with the `kill` command.

```bash
$ dura kill
```

The `kill` can happen in any directory. It indicates to the `serve`
process that it should exit if there is a `serve` process running.

## How to recover

The `dura` branch that's tracking your current uncommitted changes looks like `dura/f4a88e5ea0f1f7492845f7021ae82db70f14c725`.
In $SHELL, you can get the branch name via:

```bash
$ echo "dura/$(git rev-parse HEAD)"
```

Use `git log` or [`tig`](https://jonas.github.io/tig/) to figure out which commit you want to rollback to. Copy the hash
and then run something like

```bash
# Or, if you don't trust dura yet, `git stash`
$ git reset HEAD --hard
# get the changes into your working directory
$ git checkout $THE_HASH
# last few commands reset HEAD back to master but with changes uncommitted
$ git checkout -b temp-branch
$ git reset master
$ git checkout master
$ git branch -D temp-branch
```

If you're interested in improving this experience, [collaborate here](https://github.com/tkellogg/dura/issues/4).

## Install

### Cargo Install
1. Install Cargo  
2. If you want run release version, type ```cargo install dura``` else type ```cargo install --git https://github.com/tkellogg/dura```

### By Source

1. Install Rust (e.g., `brew install rustup && brew install rust`)
2. Clone this repository (e.g., `git clone https://github.com/tkellogg/dura.git`)
3. Navigate to repository base directory (`cd dura`)
4. Run `cargo install --path .` **Note:** If you receive a failure fetching the cargo dependencies try using the local [git client for cargo fetches](https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli). `CARGO_NET_GIT_FETCH_WITH_CLI=true cargo install --path .`

### Mac OS X

This installs `dura` and sets up a launchctl service to keep it running.

```bash
$ brew install dura
```

### Windows
1. Download [rustup-init](https://www.rust-lang.org/tools/install)
2. Clone this repository (e.g., `git clone https://github.com/tkellogg/dura.git`)
3. Navigate to repository base directory (`cd dura`)
4. Run `cargo install --path .` **Note:** If you receive a failure fetching the cargo dependencies try using the local [git client for cargo fetches](https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli). `CARGO_NET_GIT_FETCH_WITH_CLI=true cargo install --path .`

### Arch Linux

```bash
$ paru -S dura-git
```

### Nix / Nixos

[Nix][nix website] is a tool that takes a unique approach to package
management and system configuration. NixOS is a Linux distribution
built on top of the Nix package manager.

To run `dura` locally using pre-compiled binaries:

```bash
nix shell nixpkgs#dura
```

If you're willing to contribute and develop, `dura` also provides its
own ready-to-use [Nix flake][nix flake].

To build and run the latest development version of `dura` locally:

```bash
nix run github:tkellogg/dura
```

To run a development environment with the required tools
to develop:

```bash
nix develop github:tkellogg/dura
```

## FAQ

### Is this stable?

Yes. Lots of people have been using it since 2022-01-01 without issue. It uses [libgit2](https://libgit2.org/) to make the commits, so it's fairly battle hardened.

### How often does this check for changes?

Every now and then, like 5 seconds or so. Internally there's a control loop that sleeps 5 seconds between iterations, so it
runs less frequently than every 5 seconds (potentially a lot less frequently, if there's a lot of work to do).


Brought to you by <a rel="nofollow me" href="https://hachyderm.io/@kellogh">Tim Kellogg</a>.


[build badge]: https://github.com/tkellogg/dura/actions/workflows/build.yaml/badge.svg
[build action]: https://github.com/tkellogg/dura/actions/workflows/build.yaml
[nix website]: https://nixos.org/
[nix flake]: https://nixos.wiki/wiki/Flakes
