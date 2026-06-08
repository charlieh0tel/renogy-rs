---
name: release
description: Cut a renogy-rs release - bump the single workspace version, run the quality gates, commit, and tag. Use when the user wants to release, cut a version, or tag a new version. Pushing the tag (done by the user) triggers CI to build the .debs and create the GitHub Release.
allowed-tools: Bash(git status), Bash(git branch*), Bash(git rev-parse*), Bash(git log*), Bash(git fetch*), Bash(git add*), Bash(git commit*), Bash(git tag*), Bash(cargo*)
---

# Release renogy-rs

All shipped tools share one workspace version (`[workspace.package].version` in the
root `Cargo.toml`); every crate inherits it via `version.workspace = true`. A release
is a `vX.Y.Z` git tag: pushing the tag triggers CI (`.github/workflows/build-deb.yml`)
to build all four `.deb`s (amd64 + arm64) and cut a GitHub Release with auto-generated
notes. **Do not build `.deb`s locally** and **do not push** -- the user pushes the tag
themselves.

Target version: `$ARGUMENTS` (e.g. `0.3.0`). If none was given, propose the next
version from the current one and confirm before proceeding.

## Step 0 -- preconditions

- `git rev-parse --abbrev-ref HEAD` must be `main`. If not, stop and tell the user to
  release from `main`.
- `git fetch` then confirm `main` is level with `origin/main` (no unpushed commits, not
  behind). If diverged, stop and report.
- `git status` must be clean -- no staged, unstaged, or untracked files (ignore the
  known `start_aprs.sh~` backup). If anything is outstanding, stop and list it.
- Read the current version: `grep -A2 '\[workspace.package\]' Cargo.toml`. The new
  version must be a valid semver strictly greater than the current one.

## Step 1 -- quality gates (must all pass before tagging)

Run, and stop on the first failure:

```
cargo +nightly fmt --check
cargo clippy --workspace --all-targets
cargo test --workspace
cargo test -p renogy-archiver --test system -- --ignored
cargo test -p renogy-archiver-puller --test pull -- --ignored
```

The last two are the gated system tests CI also runs.

## Step 2 -- bump the version

Edit the root `Cargo.toml` only: change the `version` line under `[workspace.package]`
from the old value to the new one. This is the single source of truth; do not edit any
member crate's version.

## Step 3 -- update the lockfile

```
cargo check --workspace
```

Confirm `Cargo.lock` now shows the new version for all renogy crates.

## Step 4 -- commit

Stage exactly `Cargo.toml` and `Cargo.lock`, then:

```
git commit -m "release: vNEW"
```

## Step 5 -- tag

```
git tag vNEW
```

(Annotated is fine too: `git tag -a vNEW -m "vNEW"`.)

## Step 6 -- report and stop

Tell the user:
- the new version and tag (`vNEW`);
- that the release is **not** live until they push the tag;
- the exact command to trigger it: `git push && git push --tags`
  (or `git push origin main vNEW`);
- that CI will then build the four `.deb`s and publish the GitHub Release.

Do not run the push.
