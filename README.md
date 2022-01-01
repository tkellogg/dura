# Don't lose work!
Duralumin watches your Git repositories and makes backgound commits so that you can always recover "lost" work.

## How to use
Launch the daemon:

```
$ cd some-git-repo
$ duralumin serve
```

Then, in another console, make changes to files in the same repository. Whenever a non-ignored file is changed, you'll soon:

```
$ git log --all
```

