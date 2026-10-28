# Daily build measurements

Measured on Windows x64 with the pinned Rust 1.95.0 toolchain on October 27,
2026. The original checkout was develop at 9dd2e27e2c9af46208e804e2d33b2472faf0de38.
These are end-to-end local samples with warm dependency caches, not an isolated
clean-cache benchmark. One baseline repeat reported Cargo package-cache waits.

| Scenario (`build.cmd build`) | Original workflow | Daily workflow |
| --- | ---: | ---: |
| Unchanged source, repeated build | 57.94-62.83 s (2 samples) | 6.97-7.21 s (3 samples) |
| Small source-comment edit | 64.12 s | 10.74 s |

A final cached invocation through `dotask build` took 7.30 seconds. Across three
unchanged builds, all four tools kept the same SHA256, modification time and
embedded --version identity. The source edit refreshed the identity; its original
bytes were restored and rebuilt before handoff. Both daily and fully optimized
release verification passed all 161 Windows workspace tests, formatting, strict
Clippy, executable inspection, shim integrity and whitespace checks. Native
release creation also passed portable/self-installer and temporary install,
consumer repackaging and uninstall acceptance without changing active installs
or persistent PATH.

The original native helper only compiled/inspected release tools. The new helper
also formats and runs strict all-target Clippy before compiling/inspecting daily
tools. The repeated-build improvement comes primarily from preserving Cargo's
cache: wrappers no longer supply a different timestamp on each invocation.
Source-edit builds also benefit from incremental compilation, no optimization,
and 256 code-generation units instead of fat LTO with a single unit.

The initial dependency warm-up and the separate build driver's first compilation
are excluded. Switching profiles requires warming a separate output cache. These
numbers do not predict a fully clean build or arbitrary source changes.

## Commands and settings

```powershell
cd C:\Users\kodyb\Projects\doinstall
dotask build
dotask run --args '["--version"]'
dotask test
# Opt in to the unchanged fully optimized profile:
dotask build --profile release
dotask test --profile release
# Installer creation always uses release:
dotask create-doinstall-installer
```

Cargo-only equivalents are `build.cmd build`, `build.cmd run -- --version`,
`build.cmd verify`, and their `--profile release` variants. On Unix use
`./build.sh`. Daily `dev` settings live in the root Cargo.toml; normal Cargo
profile overrides are inherited by the task's child processes. No job limit is
introduced: Cargo retains its default logical-CPU parallelism.

The fast `bootstrap` profile keeps the running development driver outside the
`debug/` and `release/` executables that workspace tests can rebuild on Windows.
Cargo's evaluated target/build directories remain C:\tmp\_rust\doinstall\target.
Daily tools have no optimization or debug information; distributed installers
retain the original release optimization settings.

## Repeating measurements

Warm the selected workflow once, then time a few unchanged invocations:

```powershell
.\build.cmd build
1..3 | ForEach-Object {
    $elapsed = Measure-Command { .\build.cmd build }
    if ($LASTEXITCODE -ne 0) { throw 'Build failed' }
    $elapsed.TotalSeconds
}
```

The source-edit sample appended one harmless comment to src/main.rs, built once,
and restored the original bytes in a finally block. It is an invalidation probe,
not a measurement of a particular feature's implementation complexity. The
shared package build script refreshes all tools' identity when source, manifest
or lockfile inputs change. Cached builds preserve executable bytes, modification
time and embedded --version time. An explicit RUST_BUILD_TIMESTAMP is still
supported; the helper never silently replaces it with the current clock.

Observed baseline build logs/results: C:\tmp\doinstall-build-before-*. Daily
logs/results: C:\tmp\doinstall-build-after-*. These machine-local evidence files
are not repository inputs. Full daily and release verification logs are retained
separately under C:\tmp\doinstall-daily-verify.log and
C:\tmp\doinstall-release-verify.log. Native Linux/macOS execution and interactive
user acceptance are separate from these Windows measurements.
