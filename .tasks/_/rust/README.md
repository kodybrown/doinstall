# Reusable Rust tasks

These handwritten task sources are being developed here for future promotion to
dotask's official shared catalog. They are not installed catalog copies yet: no
shared-task lockfile or ownership claim is fabricated. Copy this entire directory,
including `_support/Rust.rs`, beneath another project's `.tasks/_/rust/`.
No source in this directory contains a product name, GitHub repository, SDK path,
or dependency on this repository's Cargo helper. Rust task compilation uses the
SDK bundled with the installed dotask; it does not reference the dotask checkout.

| Target | Behavior |
| --- | --- |
| `_/rust/build` | Locked Cargo build, default release profile; remap private paths and scan each reported executable |
| `_/rust/test` | Locked release-profile tests for the workspace, or the selected package |
| `_/rust/format` | Cargo formatting; `--check` checks without changes, `--format-tasks` additionally formats these task sources |
| `_/rust/clippy` | All-target Clippy checks; warnings fail |
| `_/rust/update-version` | Refresh external UTC metadata; optional version increment |
| `_/rust/tag-release` | Check clean tracked state, exact branch/HEAD/origin identity, and latest matching push CI; create an annotated tag |
| `_/rust/publish` | Check release identity/CI, refresh metadata, check formatting, lint, build/scan, test, invoke project packaging, optionally tag/push |

Cargo operations accept `--manifest`; build/test/clippy accept `--package`.
Build additionally accepts `--profile` (default `release`). A virtual workspace
needs an explicit package when building, updating or tagging its version. Keep
Cargo's evaluated target/build directories outside the checkout. The tasks do
not replace Cargo configuration or change persistent environment variables.
Shared task sources are excluded from formatting by default, preserving their
catalog ownership/hashes when copied to a project with another formatter style.
This repository's thin format wrapper opts in because it owns these originals.

`update-version` with no flags writes `rust-build/<package>.json` under Cargo's
external target directory. It records the SemVer core, human display version
`MAJOR.MINOR.PATCH+yyMM.ddhh`, full UTC date, and Unix seconds. It does not edit
Cargo's package version unless given exactly one of `--increment` (patch),
`--increment-minor` (reset patch), or `--increment-major` (reset minor/patch).
`--dry-run` changes nothing. Literal `[package]` versions and the corresponding
local package in an existing Cargo.lock are edited without rewriting surrounding
formatting, BOM, comments, or EOLs. Dependency versions are not upgraded.
Increments of inherited/workspace version
fields are refused. Increment before committing on develop.

Each build captures a fresh UTC timestamp and supplies `RUST_BUILD_TIMESTAMP`
(Unix seconds) to Cargo. A project build script must consume that contract to
embed the date in its own tools. The task's JSON alone does not alter an arbitrary
Rust program's `--version`. The date is informational, not version precedence;
core versions still need increments for new releases. Hour precision can repeat;
full UTC timestamps and source commits identify builds precisely.

`tag-release` accepts `--branch` (default `main`), `--workflow` (default
`release.yml`), `--repo` (otherwise inferred by gh), `--manifest`, and `--package`.
It requires Git, Cargo, and authenticated gh. No flags create only the local tag;
`--push` additionally pushes that single ref. `--create=false` checks/previews
without tagging/pushing, even with `--push`; it still fetches origin's branch.
Tracked changes are refused; untracked files do not enter a tag. Existing local
tags fail, and no force/replacement/deletion is performed. A failed push retains
the local tag. Git branch names containing `..` or other unsupported characters
are refused. Latest pending/failed/missing/mismatched CI stops before mutation.

`publish` uses conventional project hooks named `tag-release` and
`create-installer`. Supply those thin project tasks to configure release policy
and product-specific payload assembly, distribution notices, archive inspection
and native acceptance. The default pipeline checks the root Cargo manifest; a
project needing a selected workspace package should provide its own thin publish
orchestration with that package passed to each shared operation. The default
`publish` rehearses and packages only; `--push` tags/pushes after success. No
automatic commit, branch promotion, branch push, version increment or source
formatting is performed during publication. A CI platform matrix remains the
project workflow's responsibility; local acceptance proves only its host.

Pattern scans complement project-specific closed inventories and archive checks;
they cannot recognize every possible secret. Declare any new source companions
in task `requires` metadata before promoting this directory to the shared catalog.
