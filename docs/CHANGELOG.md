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

### 2026-10-13 Refresh the console form with an internal default theme

- Retain Cursive control behavior with a dark backdrop, contrasting form,
  space-filled colored edges, shadow, header/footer surfaces, and filled normal
  and focused buttons. Button labels use one leading/trailing space without
  angle brackets. Apply the same presentation to install/uninstall, validation,
  cancellation confirmation, warnings, errors, and summaries.
- Add optional `runtime.theme: default` as a theme identifier, with no inline
  color, padding, size, position, layout, or behavior configuration. External
  theme files remain unsupported. Input `group-index` groups related options
  with stable ordering and fixed spacing; built-in location, command, and
  shortcut groups have defaults. Omit unused presentation metadata from package
  serialization to retain existing content identity.
- Highlight complete focused checkbox labels with a visible marker and make
  their rows clickable. Disable the underlying controls as well as their wrappers
  so Tab skips dependent and CLI-locked options. Soft-wrap Unicode input values
  without inserting newlines, preserve edits/cursors on resize, and bound editor
  height with cursor-following scrolling. Keep progress text-only and wrap long
  summaries. Preserve plain/unattended output, receipts, and safe cancellation.
- Verification: release build, build.cmd verify (76 Rust tests), formatting,
  clippy, shim integrity/PE checks, and git diff --check passed on Windows x64.
  Captured actual Cursive cells/colors for focus, button, warning/error, summary,
  grouping, Unicode editing, and resize regressions. Native Windows terminal
  acceptance passed using temporary hello preview roots with PATH changes disabled,
  checking color rendering, navigation, installation/summary, cancellation,
  terminal restoration, and cleanup. Native Linux/macOS/Windows ARM64 acceptance
  remains pending. No active installation changes, integration, push, or publication.

### 2026-10-13 Explain and lock existing PATH membership in the form

- Show the PATH checkbox checked and disabled when the selected application or
  additional command directory is already on persistent user/system PATH. Explain
  why no PATH change is needed and restore the previous choice when the candidate
  moves outside PATH. Keep displayed membership separate from the submitted
  request so CLI no-PATH-write overrides remain authoritative.
- Explain CLI-locked PATH options, including the temporary hello preview's
  deliberate `--set add-to-path=false`. Preserve ownership, plain/unattended
  behavior, and the read-only PATH membership query.
- Verification: release build, build.cmd verify (78 Rust tests), formatting,
  clippy, shim integrity/PE checks, and git diff --check passed on Windows x64.
  Cursive cell/event regressions cover checked/disabled membership, its visible
  explanation, repeated refreshes, remembered true/false choices, directory
  changes, additional-command selection, and CLI locks. Membership results are
  injected without changing real PATH or active installations; native acceptance
  of this specific membership transition remains pending.

### 2026-10-13 Match the console title bars and popup layout to the reference

- Use half-block top/bottom title caps and one-column/half-row shadows on forms
  and popups, matching the supplied after screenshot. Keep popup text separated
  from the header and footer by one blank surface row above and below it.
- Move condensed keyboard instructions onto the choices button row, shortening
  hints at narrow widths without overlapping actions. Put affirmative actions
  first on Windows and negative actions first on Linux/macOS. Rename the cancel
  popup to `Confirm`; explicitly retain No as its safe default on every platform.
- Keep Cursive's control geometry and translate mouse/focus coordinates with the
  new title-cap offset. Preserve resize/edit state, PATH policy, installation
  ownership, and plain/unattended behavior.
- Verification: release build, build.cmd verify (81 Rust tests), formatting,
  clippy, shim integrity/PE checks, and git diff --check passed on Windows x64.
  Actual Cursive cell/event tests cover half-block colors, shadow offsets, blank
  popup margins, wide/compact footer alignment, both platform button orders,
  safe Enter defaults, and mouse activation after repositioning. Native Windows
  acceptance passed using temporary hello preview roots with PATH changes
  disabled, including the updated form/popup, No/Yes selection, installation,
  summary, cancellation, terminal restoration, and cleanup.
  Native Linux/macOS/Windows ARM64 acceptance remains pending.

### 2026-10-13 Simplify option groups, checkbox alignment, and text hierarchy

- Remove generated Installation, Commands, Shortcuts, and Options headings while
  retaining group ordering and blank separation. Use `Install to:` for the
  built-in form caption without replacing application-owned prompts or changing
  plain-mode questions.
- Remove checkbox focus markers and their reserved indentation. Align checkbox
  squares with text fields, retain full-row focus and clicking, and use white
  enabled checkbox labels on a darker focused surface for contrast. Descriptions,
  PATH explanations, and field captions use subdued gray; disabled controls
  remain muted. Apply the same text hierarchy to uninstall choices.
- Verification: release build, build.cmd verify (81 Rust tests), formatting,
  clippy, shim integrity/PE checks, and git diff --check passed on Windows x64.
  Updated actual Cursive rendering checks cover header removal, caption wording,
  label/description colors, checkbox alignment, focus, and row clicks; existing
  grouping, resize, cancellation, PATH policy, and worker checks remain passing.
  Native Linux/macOS/Windows ARM64 acceptance remains pending.

### 2026-10-13 Align titles and footer instructions with form content

- Move form/popup titles and the choices footer instructions one column right,
  aligning them with the content inset. Adjust title width and compact hint
  budgeting to preserve visibility and avoid overlapping buttons. Keep button
  geometry, half-block caps, shadows, spacing, and navigation unchanged.
- Verification: release build, build.cmd verify (81 Rust tests), formatting,
  clippy, shim integrity/PE checks, and git diff --check passed on Windows x64.
  Updated existing Cursive cell checks assert main title/content/hint alignment
  and popup title/text alignment; compact footer overlap checks remain passing.
  Native platform acceptance of this one-column change was not repeated.

### 2026-10-13 Keep resize notices visible and wrap long option labels

- Give the undersized-terminal notice explicit contrasting text/backdrop colors.
  Transparent layers had inherited a background style that made the notice
  invisible. Retain controls and values while either dimension is too small,
  and continue blocking installation until the form fits.
- Soft-wrap checkbox labels beside their squares and choice labels into logical
  option rows. Preserve single stored values, keyboard navigation, selection,
  disabled state, full-option focus colors, and clicking continuation lines.
  Map choice mouse coordinates and scroll focus to their logical option indices.
- Add a temporary-root Linux/macOS interactive preview with PATH locked off,
  plus WSL build, verification, unattended example, and UI preview commands.
  Use a portable bundle/native Linux clone because Debian Git cannot read the
  Windows repository's relative-worktree extension.
- Verification: release build, build.cmd verify (83 Rust tests), formatting,
  clippy, shim integrity/PE checks, and git diff --check passed on Windows x64.
  Actual Cursive cell checks cover notice contrast for narrow/short windows,
  value preservation, Unicode label wrapping, continuation clicks, disabled
  controls, choice scrolling/navigation, and reflow on resize. A native Windows
  preview passed form display, cancellation, terminal restoration, and temporary
  cleanup. Debian WSL successfully cloned/read the portable task bundle, and
  the new Unix preview passed Bash syntax checking there;
  its execution and native Linux/macOS/Windows ARM64 acceptance remain pending.

### 2026-10-13 Record Debian WSL UI acceptance and simplify preview commands

- Record the user's successful Linux x64 interactive preview in Debian WSL from
  a native Cargo debug build. Keep full Linux lifecycle/release verification,
  macOS, and Windows ARM64 acceptance separate and pending.
- Replace the required bundle/clone preview route with direct Cargo build/test
  and preview commands from the Windows source directory. Older Debian Git's
  relative-worktree limitation affects the full verification script's whitespace
  check, not Cargo compilation/tests or the interactive preview.
- Verification: documentation-only follow-up; checked the documented paths
  against the user's successful command/output and ran git diff --check.
  Source remains the previously verified UI implementation (83 Windows tests).

### 2026-10-13 Verify the console UI with included-tool installation workflows

- Reconcile the console UI branch with the committed included-tool workflows,
  preserving both feature sets and their changelog entries. Record Debian WSL
  debug-test coverage separately from the user's interactive preview and the
  pending Linux release/macOS/ARM64 acceptance.
- Verification: build.cmd verify passed 95 Rust tests, formatting, clippy, shim
  integrity/PE checks, and git diff --check on Windows x64. The release build
  passed. Native Debian WSL cargo test --package doinstall --locked passed all
  79 Linux Rust tests, including UI, included-tool, and installation lifecycle
  fixtures. Cargo targets remain external under C:/tmp/_rust/doinstall/target
  on Windows and /tmp/_rust/doinstall/target in Debian; tests use temporary roots
  and do not change the active installation or user PATH.

### 2026-10-14 Add a reusable Debian/Ubuntu web stack recipe

- Add optional doinstall-server tooling for strict stack validation, persistent
  credentials, account/database ownership, private SQL, restore-verified database
  backups, application migrations and API/nginx readiness. Keep service/package
  orchestration and editable configuration in reusable YAML tasks/templates.
- Provide Debian 12/Ubuntu 24.04 workflows for PostgreSQL, private authenticated
  Redis, nginx, systemd application services, optional static frontend, webroot
  certificates and renewal timers. Preserve immutable application versions and
  installer ownership/recovery semantics; the engine has no dotask dependency.
- Add a standalone .NET sample and frontend, document consumer packaging and
  recovery, and stage the optional helper with the build tools. Remove a
  Unix-only output-mode initialization warning without changing inventory modes;
  refresh only the recorded build-tool hash, leaving shim source/assets intact.
- Verification: build.cmd verify passed 100 Rust tests on Windows x64; native
  Linux release verification passed 88 tests in an exact-source temporary clone,
  plus formatting, clippy, shim integrity/PE and whitespace checks. Older Linux
  Git cannot read the Windows checkout's relative-worktree extension.
- Disposable Ubuntu 24.04 and Debian 12 both installed a consuming API and the
  sample API/static frontend using the same static x64 tools and pinned runtime.
  Ubuntu acceptance covered repeats, upgrade backups restored into temporary
  databases, data/credential preservation, ownership refusal, failed migration
  and unchanged-package retry, and real Windows SSH/SCP transport/execution.
  Debian also exercised dependency health failure/recovery. No production or
  user PATH changes. Native arm64 and live ACME issuance/renewal remain pending.

### 2026-10-14 Add ZIP self-extracting console launchers and configurable delivery names

- Add a precompiled native doinstall-launcher template and builder
  `--self-extracting` / `--launcher-template` options. Assemble a complete ZIP,
  strict versioned manifest, and footer onto target launcher bytes without
  recompiling application-specific code or invoking an installed archiver.
  Publish only the launcher in self-extracting mode and return its actual path
  through the existing result-file contract. Preserve fresh-output, immutable
  named-package reuse, and refusal of differing existing contents.
- Add producer-only packaging names with defaults
  `<id>-setup-v<version>-se[.exe]` and `<id>-setup-v<version>[.exe]`. Derive the
  matching YAML by stripping only `.exe` before appending `.yaml`, preserving
  dotted Unix versions. Consume packaging settings only on the build machine;
  keep installed management files, receipts, recovery, and uninstall unchanged.
  Add no compatibility loaders, fallback name searches, or migrations.
- Extract into a fresh OS temporary directory prefixed by the full launcher
  stem, or an explicit new/empty `--extract-to` directory. Intentionally retain
  complete and incomplete source directories. Preserve empty directories and
  Unix modes; verify ZIP/file hashes and reject unsupported formats, unknown
  entries, links, traversal, duplicates, and file/directory conflicts before
  launching the installer. Retain caller arguments, environment, and cwd.
- Use inherited console handles with no I/O relay. Windows waits and returns
  the installer status while allowing its cancellation/terminal cleanup;
  Linux/macOS replace the launcher with the installer using exec. Account for
  signature trailers and prepare/sign native macOS bundles with codesign.
  Stage the launcher with native build tools, configure CI tool archives that
  preserve executable permissions, and add self-extracting console previews.
  Refresh the staging build-tool hash; shim source and bundled bytes are unchanged.
- Verification: build.cmd verify passed 120 Rust tests on Windows x64;
  build.sh verify passed 102 tests in an exact-source Ubuntu x64 WSL clone,
  including formatting, clippy, shim integrity/PE, and Git whitespace checks.
  Cargo targets remained C:/tmp/_rust/doinstall/target and
  /tmp/_rust/doinstall/target. Staged release builds passed on both hosts.
  Native tests cover direct stdin/stdout/stderr, Windows console ownership,
  child exit status, Unicode/quoted arguments, matching custom YAML names,
  retained sources, corrupt containers, and installation/uninstallation.
- Native Windows and Ubuntu console previews passed form display, cancellation,
  and terminal restoration. A Linux launcher assembled by the Windows builder
  passed Linux install, validation, application launch, empty-directory retention,
  uninstall, and retained-source acceptance. Tests used disposable roots with
  PATH disabled. macOS/ARM64 execution, distribution signing/notarization,
  and remote CI were not performed.
