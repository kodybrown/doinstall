# Changelog

## Unreleased

### 2026-10-07 Extract doinstall from dotask

- Establish an independent Rust installer/builder repository from dotask commit
  27f268cee9e545a1a46292f873193732f628a219. Move the runtime, payload assembler,
  build-info support, receipt/recovery logic, native shims, tests, and authoritative
  configuration/design documentation here. Keep native shim source and delivery
  bytes unchanged; record the independent build-tool hash.
- Rename the Cargo package and release tools to doinstall and doinstall-builder.
  Provide Cargo-only build/verify/shim tooling, external evaluated output paths,
  direct-checkout instructions, and platform CI without dotask or .NET dependencies.
- Verification: build.cmd verify passed 52 Rust tests, formatting, clippy, shim
  source/asset/build-tool integrity and PE architecture/console/CLR checks. Windows
  x64 install/update/ownership/recovery/shortcut/uninstall coverage uses temporary
  roots and a disposable registry key. No active install or user PATH changed.
  Native Linux/macOS/ARM64 acceptance remains pending. No publication or push. Final committed whitespace checks passed.

### 2026-10-07 Harden standalone assembly and add a runnable consumer

- Resolve builder paths from the caller's working directory and report absolute
  artifact paths. Reject result files that overlap configuration, engine, payload,
  or completed packages, and missing result parents before publication. Report
  later result-write failures with the location of the completed package.
- Validate and distribute one staged ZIP snapshot, and check explicit output
  collisions while holding the publication lock. Preserve immutable-build,
  ownership, receipt, recovery, and uninstall formats and unchanged shim assets.
- Reject conflicting CLI actions, duplicate config/profile/input overrides, empty
  input names, and missing option values. Clarify builder help and validation
  diagnostics, replace inherited task-specific documentation with standalone CLI
  instructions, and record current release reproducibility and acceptance gaps.
- Add a tiny independent Rust consumer with Windows and Unix smoke-test scripts
  using only temporary install/command roots and disabled PATH writes.
- Verification: build.cmd build and build.cmd verify passed 55 Rust tests,
  formatting, clippy, shim integrity/PE checks, and git diff --check on Windows
  x64. New regressions cover relative paths, protected result destinations, CLI
  ambiguity, ZIP assembly bytes, and rejection before publication. The Windows
  example passed assembly, validation, install, command launch, and retained
  uninstall. Native Linux/macOS/Windows ARM64 acceptance remains pending.
