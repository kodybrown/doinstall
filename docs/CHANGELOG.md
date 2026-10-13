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

### 2026-10-09 Add a plain Cursive installer dialog

- Use Cursive's standard dialog, background, and controls over the Crossterm
  backend for interactive install/uninstall: choices, operation progress, and a
  scrollable summary. Support typed YAML fields, Tab/Shift+Tab, dependent controls,
  read-only CLI inputs, validation feedback, and resize-preserved values. No custom
  themes, UI YAML fields, or shim changes. Keep `--plain` line prompts and existing
  unattended behavior available.
- Share input precedence with the existing resolver. Run filesystem work on one
  worker thread, capture its diagnostics without writing over the active screen,
  and restore terminal state before printing results. Stop cancellation at safe
  pre-write checkpoints; complete already-started destination changes or retain
  existing retry records on failure. Preserve receipt, ownership, immutable-build,
  activation recovery, and uninstall contracts.
- Add a disposable Windows hello UI preview with PATH locked off. Align checkout
  guidance with the user-approved develop branch and document interaction,
  cancellation, sizing, and native acceptance limits.
- Verification: build.cmd build, build.cmd verify (64 Rust tests), formatting,
  clippy, shim integrity/PE checks, and git diff --check passed on Windows x64.
  Nine UI tests cover event dispatch, dependencies, validation, size gating,
  cancellation, ownership failures, and temporary-root install/uninstall. Native
  Windows terminal acceptance covered choices/progress/summary, command launch,
  retained uninstall, Ctrl+C cleanup, and the preview script. Resize behavior was
  tested headlessly; native Linux/macOS/Windows ARM64 acceptance remains pending.

### 2026-10-11 Grow installer dialogs with the console

- Remove the fixed 72-column dialog limit. Forms now fill the console's usable
  width with a small margin, giving editable paths more space when the terminal
  grows or is maximized. Height follows content up to the available space;
  shorter progress and summary screens remain compact. Retain scrolling,
  minimum-size gating, field values, and cursor position across resizes.
- Verification: build.cmd build and build.cmd verify passed 65 Rust tests,
  formatting, clippy, shim integrity/PE checks, and git diff --check on Windows
  x64. A rendered-control resize regression failed against the previous cap and
  passed after the change, covering expansion from 90 to 160 columns and shrinking
  back without losing values or cursor position. Native terminal resize and
  Linux/macOS/Windows ARM64 acceptance remain pending.

### 2026-10-11 Confirm interactive cancellation by default

- Add boolean YAML `runtime.confirm-cancel`, defaulting to true, for install and
  uninstall dialogs. Cancel, Escape, and Ctrl+C ask before canceling choices or
  signaling a working operation; No is focused by default. Enter/No/Escape resumes
  the dialog with its values intact. False retains immediate cancellation requests.
  Closing a summary does not prompt, and worker completion dismisses any pending
  confirmation before showing its result.
- Separate external interrupt requests from worker cancellation and replace
  Cursive's priority Ctrl+C quit callback so confirmation cannot be bypassed.
  Preserve safe pre-write cancellation, forward recovery, and unattended/plain
  behavior. Omit the default policy from serialized YAML to retain existing
  package content identity when the option is not customized.
- Verification: build.cmd build, build.cmd verify (70 Rust tests), formatting,
  clippy, shim integrity/PE checks, and git diff --check passed on Windows x64.
  Focused tests cover default/false YAML serialization, No/Yes/Escape/Ctrl+C,
  disabled confirmation, external interrupts, and completion while a prompt is
  open. Native Windows terminal acceptance checked cancellation confirmation
  and terminal restoration using the temporary hello preview with PATH disabled.
  Native Linux/macOS/Windows ARM64 acceptance remains pending.

### 2026-10-12 Deliver prebuilt tools with a declarative dotask adapter

- Add optional `adapters/dotask/assemble.task`, invoking the builder directly
  through dotask's process-step format with explicit builder, installer, config,
  and fresh output paths. Document consumer installation, target defaults, and
  resolved YAML/payload requirements. Keep producer steps and typed
  InstallerArtifact integration in the caller's existing adapters.
- Stage a Windows x64 tool bundle with both release executables, the task,
  documentation, and license. DoInstall's build/runtime retain no dotask or .NET
  dependency. The older installed dotask does not understand process-task options;
  verification used the current source CLI without changing the active install
  or PATH, and documents the required direct-process support.
- Verification: build.cmd build and build.cmd verify passed 70 Rust tests,
  formatting, clippy, shim integrity/PE checks, and git diff --check. Native
  Windows x64 acceptance invoked the .task using project defaults and paths with
  spaces/Unicode, assembled and validated a package, installed and launched a
  temporary command, uninstalled, and refused an existing output without changing
  it. Staged executable hashes matched evaluated external Cargo release outputs.
  Native Linux/macOS/Windows ARM64 acceptance remains pending.

### 2026-10-12 Keep build-system adapters with their consuming tool

- Remove the optional dotask task and its adapter-specific documentation from
  this repository. Dotask now owns its optional shared task, distribution,
  configuration, and updates, like its other external-utility adapters. Doinstall
  remains a separately supplied runtime/builder pair with an ordinary CLI; it
  never installs project task files or depends on dotask.
- Preserve all executable, YAML, package, receipt, and activation behavior.
  Verification: build.cmd build, build.cmd verify (70 Rust tests), formatting,
  clippy, shim integrity/PE checks, and git diff --check passed on Windows x64.
  Native Linux/macOS/Windows ARM64 acceptance remains pending. No active
  installation, publication or push occurred.

### 2026-10-13 Included-tool lifecycle and destination gates

- Launch pinned tools bundled in immutable payloads through prepare,
  before-activate, and after-activate phases. Provide a temporary structured
  context, exact argv/env tokens, process-tree timeout/cancellation cleanup, and
  non-interactive execution without a dotask/source/toolchain dependency.
- Gate destination platform/architecture, explicit Debian/Ubuntu versions, and
  optional root/systemd requirements before installation writes. Separate builder
  validation from destination checks and verify the supplied engine's native
  format/architecture without executing it.
- Carry Unix executable modes and empty directories in ZIPs assembled on Windows,
  use destination engine filenames, and preserve repeated named-package reuse.
- Refuse adoption of unowned external configuration, verify owned hashes/modes,
  retain restricted configuration snapshots and workflow journals, and stop before
  successful receipt finalization/pruning on failed hooks. Original-package/input
  retries remain explicit; automatic rollback and service cleanup are deferred.
- Resolve the development helper's checkout at runtime so shared cached Cargo
  artifacts cannot redirect verification into another worktree. Refresh only its
  integrity fingerprint after verifying unchanged shim source/assets.
- Verification: build.cmd verify passed all 82 Rust tests, formatting, clippy,
  shim integrity/PE checks, and git diff --check in this develop checkout on
  Windows x64. Ten new native fixtures cover tool execution, failure/retry phases,
  guarded config ownership, timeout descendant cleanup, input preflight, and Linux
  ZIP assembly without running the destination engine. Cargo output remains
  C:/tmp/_rust/doinstall/target. Native Debian/Ubuntu/macOS/ARM64 acceptance remains
  pending; fixtures use disposable directories and do not provision live servers.
