# Don't lose work!
Dura watches your Git repositories and makes backgound commits so that you can always recover "lost" work.

## How to use
Launch the daemon:

```
$ cd some-git-repo
$ dura serve
```

The `serve` can happen in any directory, but you need to tell `dura` to watch directories that contain Git repos:

```
$ dura watch
# ... make some chanes, wait ...
$ git log --all
```

You should see a branch called something like `dura-49a103a09c509aa3c9ed90126a6fc10a686c8bf1` where the `49a10...` hash
is HEAD, the most recent commit in the current branch.

## Install

1. Install rust (e.g. `brew install rustup`)
2. Clone this repository 
3. Run `cargo install --path .`


# FAQ
## Is this stable?
It's still in prototype phase. Open issues pertaining to stability are marked with the 
[stability](https://github.com/tkellogg/dura/issues?q=is%3Aopen+is%3Aissue+label%3Astability) tag. 

## How often does this check for changes?
Every now and then, like 5 seconds or so. Internally there's a control loop that sleeps 5 seconds between loops, so it runs less than
every 5 seconds (potentially a lot less, if there's a lot of work to do).

