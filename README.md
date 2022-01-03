Dura is a background process that watches your Git repositories and makes hidden commits. If you ever get into an "oh snap!" situation
where you think you just lost days of work, checkout a `dura` branch and recover.

Without `dura`, you use Ctrl-Z in your editor to get back to a good state. Thats's so 2021. Computers crash, and Crl-Z only works on files 
independently. Dura snapshots changes across the entire repository as-you-go, so you can revert to "4 hours ago" instead of "hit Ctrl-Z 
like 40 times or whatever". Finally some sanity.

## How to use
Launch the daemon:

```
$ dura serve &
```

The `serve` can happen in any directory. The `&` is bash syntax to "daemonize" the process, meaning that you can start `dura` and then 
keep using the same terminal window while `dura` keeps running. You could also run `dura serve` in a window that you keep open.

Let `dura` know which repositories to watch: 

```
$ cd some/git/repo
$ dura watch
```

Right now you have to `cd` into each repo that you want to watch, one-at-a-time. If you have thoughts on how to do this better, share them [here](https://github.com/tkellogg/dura/issues/3).

Make some changes. No need to commit or even stage them. Use any Git tool to see the `dura` branches:

```
$ git log --all
```

`dura` produces a branch for every real commit you make and makes commits to that branch without impacting your working copy. You
keep using Git exactly like you did before.

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

## Does this work on my OS?
* Mac: yes
* Linux: probably
* Windows: possibly

