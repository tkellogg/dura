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

1. Install rust (e.g. `brew install rust`)
2. Clone this repository 
3. Run `cargo install --path .`


# FAQ
## Is this stable?
lol no

## How often does this check for changes?
Every now and then, like 5 seconds or so.

