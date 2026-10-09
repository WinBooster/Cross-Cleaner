# 🤝 Contributing to Cross Cleaner

Thanks for taking the time to contribute! This file covers how to build, what a
good pull request looks like here, and — the part that is easy to get wrong —
**how to write commits, because the commit messages are the changelog**.

For build, packaging and cross-compilation details see [BUILD.md](BUILD.md).

## 🚀 Before you start

```bash
git clone https://github.com/WinBooster/Cross-Cleaner.git
cd Cross-Cleaner
cargo build          # debug build
cargo test           # run the test suite
```

Rust 1.70 or higher is required. CI (`.github/workflows/dev_build.yml`) runs on
every pull request and push that touches Rust, Cargo, JSON or workflow files.

## ✅ Before you open a pull request

```bash
cargo fmt            # run it locally; CI reformats for you and commits the result
cargo test
cargo build          # make sure the workspace still compiles
```

## 🌿 Branches and pull requests

- Branch off `main`, one topic per pull request.
- Keep the PR title specific and user-facing, and describe what changed and how
  to verify it. Mention platform-specific effects: this project builds for
  Windows, Linux, macOS and Android, and a change that only makes sense on one
  of them should say so.
- Squash-merge is fine. If you keep the individual commits, make sure each one
  follows the format below — **the release notes are built from the commits
  between the previous release tag and the new one**, so every commit that is
  not `feat`/`fix`/`perf` still shows up, under its type.

## 📝 Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<optional scope>): <description>
```

- **type** — one of the types listed below, lowercase, from a fixed set.
- **scope** — optional area of the codebase, e.g. `tui`, `cli`, `database`.
- **description** — imperative mood, lowercase, no trailing period:
  `fix: guard against an empty program list`, not `Fixed the bug.`

A change that breaks backwards compatibility adds `!` before the colon and a
`BREAKING CHANGE:` paragraph in the body:

```
feat(api)!: drop the legacy --quick flag

BREAKING CHANGE: --quick was removed. Use --dry-run instead, which does the
same without deleting anything.
```

### All commit types

Every type is recognised. The "Changelog section" column is where the commit
ends up in the release notes.

| Type | Changelog section | Use it for |
| --- | --- | --- |
| `feat` | **Features** | A new user-facing capability. |
| `fix` | **Bug Fixes** | A bug that users can hit. |
| `perf` | **Performance** | A speed, memory or size improvement. |
| `chore` | Other Changes | Repo hygiene that changes no behaviour. See below. |
| `build` | Other Changes | Build system, Cargo features, packaging. |
| `ci` | Other Changes | CI configuration and workflows. |
| `docs` | Other Changes | Documentation only. |
| `refactor` | Other Changes | Code restructuring with no behaviour change. |
| `test` | Other Changes | Adding or fixing tests. |
| `style` | Other Changes | Manual formatting tweaks. Prefer `chore: apply rustfmt`. |
| `revert` | Other Changes | Reverting an earlier commit. |
| *(breaking)* | **Breaking Changes** | Any of the above with `!` or `BREAKING CHANGE:`. |

### `feat` — new capability

```
feat: add arm64 installer for Windows
feat(cli): add --json output for machine-readable results
feat(database): add macOS Safari cache entries
feat: mouse interaction in the program list
```

Reach for `feat` only if a user could describe the change as "it can do
something new now". If nothing new is possible, it is a `fix` or a `chore`.

### `fix` — something was broken

```
fix: guard against an empty program list
fix(tui): fix encoding of non-ASCII program names
fix(ui): fix next button in TUI
fix: crash when scanning a network share
```

### `perf` — same behaviour, faster

```
perf: parallelize directory scanning
perf(database): cache parsed JSON between runs
perf(ui): lazy-load program details
```

Note that `perf` gets its own **Performance** section in the changelog, so a
real improvement is worth the effort to label correctly.

### `chore` — maintenance that touches no behaviour

This is the type most often misused, so it is spelled out here. Use `chore`
when the repository, the tooling or the dependencies change but **no code path
does**. If a user could notice the difference in any way, it is not a `chore`.

| What changed | Example |
| --- | --- |
| Formatting applied by the toolchain | `chore: apply rustfmt` |
| Dependency version bump | `chore(deps): bump tokio from 1.53.1 to 1.53.2` |
| Editor / tooling config | `chore: configure rust-analyzer in .zed` |
| Ignore files | `chore: ignore .idea and target in .gitignore` |
| Data file housekeeping | `chore(database): sort programs by name` |
| Comment or whitespace cleanup | `chore: fix trailing whitespace in comments` |
| Release metadata | `chore(release): 2.0.4.2.2 [skip ci]` |

The CI jobs commit these themselves, so do not repeat them by hand:

- `chore: apply rustfmt` — the `formating_code` job
- `chore(database): sort programs by name` — the `sort_json` job
- `docs: update program catalogs` — `update_program_lists.yml`

Because `chore` goes into **Other Changes**, it never appears under Features or
Bug Fixes. A dependency bump that is worth telling users about — because it
unblocks a feature or fixes a CVE — deserves a line in the PR description
instead.

> Automated commits from `github-actions[bot]`, `dependabot` and similar
> accounts are filtered out of the sections above entirely. They are still
> listed in the **Commit history** table at the bottom of the release notes, so
> the automated work stays auditable.

### `build` — how it is built

```
build: use Inno Setup 7
build: drop the aarch64-linux-android target from CI
build(deps): bump crossterm to 0.28
build(linux): add AppImage packaging
```

Use `build` when the change is about compiling, linking or packaging. Keep
`build(deps)` for dependency bumps that come with the build system; a plain
runtime dependency bump is usually better as `chore(deps)`.

### `ci` — the pipeline itself

```
ci: run clippy on pull requests
ci(ci): cache cargo registry between runs
ci: run tests on windows-11-arm
```

`ci` is about the workflow that checks the code, not about the code. A change to
`dev_build.yml` that only affects which runners are used is `ci`; one that
changes what gets compiled or shipped is `build`.

### `docs` — documentation only

```
docs: document --no-default-features in BUILD.md
docs: add arm64 install instructions for winget
docs: explain the self-update opt-out in README.md
```

### `refactor` — restructure without changing behaviour

```
refactor: move path matching into the database crate
refactor(tui): split the rendering code out of main.rs
refactor: replace the manual event loop with a select
```

If a refactor is observable by users, it is not a refactor — describe the effect
in the description, or use `feat`/`fix`.

### `test` — tests only

```
test: add proptest cases for UNC paths
test(tui): cover the empty program list
test: fix flaky installer test on windows-latest
```

### `style` — manual formatting

```
style: wrap the long matcher chain in database/src/lib.rs
```

Prefer letting `cargo fmt` do it and letting CI commit
`chore: apply rustfmt`; reach for `style` only when a change is purely about
how the code reads.

### `revert` — undoing something

```
revert: drop the experimental --force flag
revert: "feat(cli): add --json output"
```

### Breaking changes

Either form works; `!` in the header plus a `BREAKING CHANGE:` paragraph in the
body is the clearest:

```
feat(cli)!: rename --dry-run to --plan

BREAKING CHANGE: --dry-run is now --plan. The old name is removed.
```

```
fix: handle read-only databases

BREAKING CHANGE: the cleaner no longer requests elevation when the target
directory is read-only; it reports the skipped entries instead.
```

Breaking changes get their own section at the top of the release page, so use
them only for changes that really do require users to act.

### Scopes used in this repository

A scope narrows a commit to an area. Common ones here:

| Scope | Area |
| --- | --- |
| `cli` | `crates/cli` |
| `tui` | `crates/tui` |
| `ui` / `gui` | `crates/gui`, `crates/desktop` |
| `database` | `crates/database` and the `*_database.json` files |
| `cleaner` | `crates/cleaner` |
| `android` | `crates/android` |
| `selfupdate` | `crates/selfupdate` |
| `build` | `Cargo.toml` features and dependencies |
| `ci` / `release` | `.github/workflows` |
| `deps` | dependency version bumps |
| `windows` / `linux` / `macos` | platform-specific behaviour |

Scopes are free-form and only used for readability. Keep them short and
lowercase.

### Rules that decide whether your commit reaches users

1. **Never rewrite `main` history between releases.** A `rebase` or a force push
   after the previous tag drops commits out of the `previous tag → new tag`
   range, and those commits then never appear in the notes.
2. **Describe the effect, not the diff.** `fix: fix crash when scanning a
   network share` is useful; `fix: fix` is not.
3. **One concern per commit.** A commit that fixes a bug and reformats half the
   workspace cannot be classified, so it lands in Other Changes and the fix goes
   unmentioned.

## 📦 Generated files — do not edit by hand

A few files are produced by CI and will be overwritten:

| File | Generated by |
| --- | --- |
| `LIST_WINDOWS.md`, `LIST_REGISTRY.md`, `LIST_LINUX.md`, `LIST_MACOS.md`, `LIST_ANDROID.md`, `LIST_CUSTOM.md` | `.github/workflows/update_program_lists.yml` |
| `version =` lines in `Cargo.toml` / `Cargo.lock` | the `formating_code` job of the release workflow |
| Sorting of `crates/database/*_database.json` | the `sort_json` job (sorted by `program`, case-insensitive) |

Edit the sources instead:

- program catalogs: `crates/database/*_database.json`
- built-in custom cleanings: `crates/cleaner/src/custom_cleaners.rs`
- `LIST_CUSTOM.md` and the "Built-in custom cleanings" sections are parsed out
  of `custom_cleaners.rs` by a regex, so keep the `id` / `program` /
  `category` / `path` field formatting in that file intact.
- links for the catalog tables: `crates/database/program_links.json`

## 🚢 Release process (maintainers)

1. Run the **Release** workflow with the new version. It bumps the version in
   the `Cargo.toml` files, reformats, sorts the databases, builds every
   platform, then creates the tag and a **draft** release whose notes are
   generated from the Conventional Commits since the previous tag.
2. Review the assets and the notes on the draft, edit if needed, then publish it
   on the releases page.
3. Run the **Publish Release** workflow with the same version to submit the
   installer to winget and regenerate the `LIST_*.md` catalogs.

Versions have four components (`2.0.4.2.1`). The tag and the release use the
full version; the `Cargo.toml` files use the first three, because Cargo requires
semver.

Commits that predate this convention (`Fix encoding`, `x64 fixes`, …) are
listed under **Other Changes** rather than blocking the release. New commits are
expected to follow the format above.

The release notes end with a collapsed **Commit history** table listing every
commit in the release range with its author and a link to the diff, so a
reviewer can check what actually landed before publishing the draft.

`secrets.WINGET_TOKEN` must be available in the repository for step 3.

## 📄 License

By contributing you agree that your contributions are licensed under the
[GNU General Public License v3.0](LICENSE).