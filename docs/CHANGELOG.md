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

### 2026-10-16 Keep rustfmt configuration quiet on stable Rust

- Comment out 52 nightly-only rustfmt settings while preserving their preferred
  values and explanatory comments. Keep all 27 supported settings active and
  unchanged, including Rust edition, style edition, indentation, and line width.
  Document stable rustfmt as the repository formatter without changing the
  installed toolchain or user-level configuration.
- Verification: cargo fmt --all -- --check passed with zero warnings, compared
  with 520 warning lines before the change. No Rust source files changed;
  git diff --check passed. Cargo's evaluated target directory remains external
  at C:/tmp/_rust/doinstall/target. Runtime tests were not repeated for this
  configuration-only change; the running installer was left undisturbed.

### 2026-10-16 Revalidate self-extracting delivery for develop integration

- Rebase the self-extracting launcher and stable formatter changes onto the
  current develop baseline. The only additional baseline change is a Git
  ignore rule; the previously tested implementation remains unchanged.
- Verification: build.cmd verify passed all 120 Rust tests on Windows x64,
  formatting with zero warnings, clippy, shim integrity/PE checks, and
  git diff --check. Cargo's evaluated target directory remains external at
  C:/tmp/_rust/doinstall/target. Tests use disposable installation roots.
- The earlier exact-source Ubuntu x64 verification passed 102 tests. A repeat
  Linux run could not start because the acceptance WSL distro's disk file is
  missing. Native macOS/ARM64, distribution signing/notarization, and remote CI
  remain unverified.

### 2026-10-16 Document self-extracting package creation for consumers and AI assistants

- Add a complete build-machine recipe with minimal resolved YAML, prebuilt host
  and target tool selection, the builder command, result FilePath consumption,
  retained extraction, validation, and the exact distributable. Link it from the
  README and agent instructions, and point dotask consumers to its separate
  adapter recipe. Keep the runtime/packaging contract in INSTALLATION.md.
- Verification: execute the documented Windows PowerShell/YAML recipe using
  prebuilt tools in a disposable project. Package assembly, custom extraction,
  matching-name YAML, and --validate passed without installation or PATH changes.
  Git whitespace and local documentation links/anchors passed. No Rust source
  changed; runtime tests were not repeated for this documentation-only update.

### 2026-10-16 Package doinstall using its own self-extracting installer

- Add build package with output-parent/version overrides and a standalone
  packaging/doinstall.yaml recipe. Snapshot all four native tools and their
  license, assemble them through the public builder, and print the actual
  self-extracting artifact path. Derive the default version from Cargo metadata;
  preserve immutable output refusal. Packaging never installs the result. Keep
  helper comments explicit about bootstrapping and freshly built product tools.
- Install owned runtime/builder/server commands with additional commands enabled
  by default. Keep the unassembled launcher template beside the real builder.
  Use ordinary receipts, immutable builds, activation, upgrades, and retained
  uninstall; refuse unrecognized manually copied state without adoption/migration.
- Add builder --print-tool-paths to report validated native Installer and
  LauncherTemplate sibling paths through command shims. Keep it independent of
  task systems, registry/receipt searches, and version-directory discovery.
  Document one-command creation, installation, discovery and uninstall; configure
  native CI installer archives alongside the existing tool artifacts.
- Verification: build.cmd verify passed all 122 Rust tests on Windows x64,
  formatting, clippy, shim integrity/PE and Git whitespace checks. New lifecycle
  acceptance installed tools, ran command shims, packaged another self-installer
  using installed tools, upgraded, retained the previous build, and uninstalled.
  Query tests covered missing siblings and mixed-command refusal. Refresh only
  the build-helper hash; shim source and both bundled shim binaries are unchanged.
- build.cmd package and release staging passed. The actual installer displayed
  its themed form and canceled with terminal restoration and retained sources.
  A temporary installation exposed its builder through a child-only PATH entry;
  dotask created/validated an application launcher without tool-path settings,
  then the tools were uninstalled. Persistent user/machine PATH stayed unchanged.
  Installer tar creation and 35 local documentation links/anchors passed. Cargo
  output remains C:/tmp/_rust/doinstall/target. Native Linux execution of this
  workflow and remote CI remain unverified; macOS/ARM64 acceptance is deferred.
  No active installation, publication, or push was performed.

### 2026-10-19 Initialize editable application settings from bundled defaults

- Add installer-native runtime.settings-files for YAML/JSON defaults and typed
  input-to-scalar JSON-pointer mappings. Require if-exists: preserve, validate
  sources/paths/inputs before writes, and publish new settings exclusively and
  atomically outside immutable application builds. Plain seeds preserve source
  comments, UTF-8 BOM, and line endings; generated overrides safely retain literal
  application tokens without interpolating serialized document text.
- Add expand: false for custom string inputs in CLI, plain prompts, and Cursive
  forms. Keep existing input expansion/default serialization unchanged when the
  feature is unused. No application-specific logic, dotask dependency, hooks,
  compatibility loader, or migration is added to doinstall.
- Preserve existing settings byte-for-byte on install/update without adopting
  unowned files. Record only newly created files for explicit settings removal,
  preserve them by default on uninstall, coordinate shared destinations, and
  recover recorded settings ownership during pending activation retries.
- Verification: build.cmd verify passed 133 Rust tests, formatting, strict Clippy,
  shim source/asset/PE integrity, and Git whitespace checks on Windows x64. Eight
  new settings-file integration tests cover safe initialization, literal/custom
  values, upgrade preservation, seed fidelity, validation refusal, JSON output,
  uninstall ownership, and retry recovery; plain/dialog literal input tests pass.
  Filekeep's self-extracting consumer passed temporary-root install, custom
  configuration, backup, reinstall/upgrade, and retained uninstall acceptance with
  persistent user/machine PATH unchanged. Native Unix/ARM64 acceptance is unverified.

### 2026-10-19 Native app-data paths and plain-text editable settings

- Add `${app-data}` for Roaming AppData on Windows and XDG configuration on Unix,
  honoring absolute redirected environment paths and falling back to the user's
  platform configuration directory. Keep the existing `${config-home}` contract.
- Support `format: text` settings defaults with byte-exact copying and preservation
  of existing user files. Reject structured `set` mappings for text before writes.
- Document the native path token and INI/application-specific settings recipe.
- Verification: focused settings/path tests and `build.cmd verify` passed on
  Windows x64, including native install/upgrade/uninstall with temporary redirected
  app-data roots, encoding/BOM/EOL preservation, and read-only validation. Evaluated
  Cargo target/build directories remain `C:/tmp/_rust/doinstall/target`. No active
  tool installation or persistent PATH changed; Unix native acceptance not run.

### 2026-10-19 Prepare versioned GitHub release automation

- Add Windows x64 and Ubuntu 22.04 x64 verification, native delivery acceptance,
  portable ZIP/tar.gz bundles and self-extracting installers using the existing
  Cargo build helper and public packaging recipe. Preserve macOS source checks
  and the separate shim compilation gate. Manual/push/PR builds upload artifacts;
  only exact Cargo-version tag pushes can publish after all gates pass.
- Pin Rust 1.95.0 and Actions commits. Keep build/collection jobs read-only and
  grant contents write only to draft-upload/publication. Refuse replacement of
  existing releases, inconsistent platform metadata, dirty release inputs and
  checksum/inventory mismatches. Keep all build and release outputs external.
- Bundle matching runtime, builder, launcher template and server with project and
  dependency license texts, source commit/toolchain metadata, stable versioned
  asset names and combined SHA256SUMS. Add build-helper result FilePath reporting
  and checked distribution metadata staging; preserve ordinary source packaging.
- Remap private compile paths, neutralize Windows PDB paths, strip symbols and
  inspect decompressed archives, installer contents and all delivered binaries
  for private paths and credential patterns. Preserve Linux executable modes and
  require the Ubuntu 22.04 / glibc 2.35 build baseline and symbol ceiling.
- Document exact assets, pinned download/hash verification, adjacent-tool
  discovery, the release/rehearsal procedure, default-branch dispatch prerequisite,
  compatibility limits and separate local versus hosted acceptance evidence.
- Verification: build.cmd verify passed 137 Rust tests on Windows x64;
  build.sh verify passed 119 on native Ubuntu 22.04.5 x64 in Docker, including
  formatting, strict clippy, shim integrity and whitespace. Five release boundary
  tests passed on both platforms; actionlint 1.7.11 and two-platform checksum
  collection passed. Finished portable bundles and self-installers passed native
  temporary-root discovery/install/payload-hash checks, downstream self-extracting
  assembly/launch and uninstall on both platforms. Active installations and
  persistent PATH were not changed. Cargo target/build directories remain
  C:/tmp/_rust/doinstall/target and /tmp/_rust/doinstall/target respectively.
  Hosted Actions/macOS execution, signing and publication remain unverified;
  no remote push or release tag was created.

### 2026-10-19 Limit branch CI to main and resolve hosted fixture paths

- Run automatic branch builds only for main; remove pull-request and other-branch
  triggers. Preserve explicit version-tag releases, requiring exact Cargo-version
  tags whose commits are already reachable from remote main. Allow manual builds
  only on main, with reference validation before compilation or packaging.
- Diagnose the first hosted failures: Windows UI fixtures compared runneradmin
  paths against RUNNER~1 aliases and used mismatched receipt roots; macOS fixtures
  traversed /var rather than /private/var. Resolve temporary fixture parents before
  recording expected paths or invoking engine operations. Share physical-path
  setup across integration fixtures and normalize synthetic self-extracting unit
  fixtures. Preserve runtime refusal of links and junctions inside owned trees.
- Update release procedures and acceptance evidence. The initial hosted main run
  passed Linux delivery acceptance and both shim builds; revised Windows/macOS
  hosted verification and release publication still require a new main push.
- Verification: build.cmd verify passed all 137 Windows Rust tests, formatting,
  strict clippy, shim integrity and whitespace with child-only TEMP/TMP redirected
  through the C:/tmp junction. Five release boundary tests, actionlint 1.7.11 and
  local documentation links passed. Cargo target/build directories remain
  C:/tmp/_rust/doinstall/target. Persistent PATH and active installations were
  unchanged. No push, tag, hosted rerun or publication was performed.

### 2026-10-19 Correct Mach-O preparation for self-extracting code signing

- Inspect hosted main run 39341579459: Windows/Linux delivery acceptance and
  shim builds passed, and the macOS path-fixture tests passed. Seven macOS
  self-extracting integration tests then failed when codesign rejected the
  completed launcher as an invalid signature format.
- Remove the old Mach-O LC_CODE_SIGNATURE command together with its signature
  blob instead of leaving an empty command for Apple's signer to parse. Compact
  load commands into their existing header space without moving native data or
  changing existing code/data offsets.
- Declare appended container bytes in the trailing string-table range and
  __LINKEDIT segment, preserving existing symbol offsets and allowing Apple's
  signing allocator to retain all delivery bytes. Refuse unsupported table
  layouts and stale commands before updating the file. Native macOS assembly
  now verifies the completed ad-hoc signature with codesign --verify --strict
  before checking the embedded bundle. Keep the existing container format.
- Document the Mach-O signing contract and distinguish observed hosted success
  from pending native acceptance. Add three portable structural regression tests
  for signature-command removal, x64/ARM64 link-edit/table extents, preserved
  data/offsets, new signature trailers and malformed-input refusal.
- Verification: build.cmd verify passed all 140 Windows Rust tests, formatting,
  strict clippy, shim integrity and whitespace. Five release boundary tests,
  actionlint 1.7.11 and documentation links passed. Cargo target/build directories
  remain C:/tmp/_rust/doinstall/target. Native macOS code signing/execution still
  needs a hosted rerun; Windows structural tests do not prove it. Persistent PATH
  and active installations were unchanged; no push, tag or publication occurred.

### 2026-10-20 Temporarily disable macOS installer CI

- Remove the macOS verification job, which builds tools and generates installers
  during native acceptance tests. Remove its dependency from release collection;
  Windows x64, Linux x64 and the shim-source check remain required gates.
- Keep macOS source/signing code for future work, but document that macOS CI and
  installer acceptance are disabled and its native packaging remains unverified.
  Preserve main-only branch triggers, explicit release-tag validation, manual
  main builds, permissions and the Windows/Linux asset contract.
- Verification: actionlint 1.7.11, five release boundary tests, local documentation
  links and git diff --check passed. No Rust source or build commands changed;
  native test suites were not repeated. No push, tag or publication was performed.

### 2026-10-20 Add optional YAML release-tag task

- Add .tasks/tag-release.task using native dotask workflow steps. Derive the
  version from the existing release validator, require clean tracked files and
  identical HEAD/local main/fetched origin/main commits, and require the latest
  main push workflow to have succeeded for that commit before tagging it.
- Create an annotated local tag by default; --push sends only that new release
  tag. --create=false checks and previews without tagging or pushing, including
  when combined with --push. Never force, replace or delete tags. Keep failed
  pushes recoverable by leaving the local tag available for inspection.
- Add exact-token output to the existing Python version command for task process
  capture. Document prerequisites, commands, guards and manual alternatives.
  Keep build/runtime/release CI independent of dotask; preserve user configuration.
- Verification: build.cmd verify passed all 140 Windows Rust tests, formatting,
  strict clippy and shim integrity. All 13 Python tests passed, including optional
  installed-dotask execution with simulated Git/GitHub tools for preview, tagging,
  push and failure paths. Real GitHub query validation, task help, documentation
  links and git diff --check passed. Cargo target/build directories remain
  C:/tmp/_rust/doinstall/target. Linux task execution is not claimed. No actual
  tags, pushes or releases were created; active installations and PATH were unchanged.

### 2026-10-20 Add reusable Rust tasks and remove Python release tooling

- Develop reusable Cargo build, test, format, Clippy, update-version, tag-release
  and publish tasks under .tasks/_/rust, including their declared Rust support
  files. Keep thin project entry points and doinstall-specific self-packaging at
  the task root. Do not hardcode product/repository names into reusable code or
  fabricate shared-catalog ownership; future catalog promotion remains separate.
- Replace all four Python release/test files with Cargo-bootstrapped Rust release
  assembly, boundary tests, native acceptance and artifact collection. Actions
  invokes Cargo directly, retaining main-only triggers, Windows/Linux gates,
  pinned tools/actions, read-only builds and a separate release-writing publisher.
  Record the no-Python policy in AGENTS.md; installed dotask/.NET remain optional.
- Give all four native tools --version output with a shared UTC build identity:
  MAJOR.MINOR.PATCH+yyMM.ddhh and full UTC time. Keep Cargo, tags and asset/install
  versions on the stable SemVer core. Refresh generated metadata outside source;
  optional patch/minor/major increments edit only literal package versions and
  their local lockfile entry, preserving comments/BOM/EOLs and dependencies.
- Make task/build-helper builds remap private paths and scan resulting binaries.
  Preserve closed payload inventories, dependency notices, archive traversal/link
  checks, Linux executable modes, matching adjacent tool discovery, installed
  payload hashes and native self-installer/consumer-installer acceptance. Native
  assembly uses the current source build and builder-returned FilePath, never an
  active installation. Generate ZIP/tar.gz locally or in Actions, outside source.
- Publish rehearses by default. Require matching clean main/main CI, then check
  formatting, lint, build/scan, test, package/inspect/exercise deliveries and only
  then tag/push with explicit --push. Never commit, promote, push branches, force
  or delete tags automatically. Shared formatting leaves catalog task copies
  alone unless explicitly requested; this project's wrapper formats its originals.
- Verification: build.cmd verify passed 152 Windows Rust tests and strict Clippy,
  formatting, shim integrity and whitespace. Four shared-helper unit tests passed.
  Installed-dotask disposable-project acceptance covered independent task copies,
  increments/previews, all tag guards, existing tags, failed pushes, binary private
  content, publish ordering, packaging failure and safe default publication; Git
  and GitHub mutations were simulated. Ubuntu 22.04/glibc 2.35 Docker verification
  passed 133 native tests and Clippy; optional installed-dotask acceptance skipped
  there. ZIP/tar.gz and self-installers passed native temporary-root acceptance on
  both platforms; combined asset/checksum collection passed. Linux dotask task
  execution and macOS/ARM64 native acceptance are not claimed.
- Rebuilt both shim architectures with Ubuntu LLVM 14 as a compilation check;
  retain the original LLVM 22 delivery bytes and native Windows x64 acceptance.
  Refresh the Rust build-helper integrity hash; do not claim byte-identical
  reproduction across LLVM versions. Actionlint 1.7.11 and documentation links
  passed. Cargo target/build directories remain C:/tmp/_rust/doinstall/target.
  Preserve user configuration and persistent PATH/active installations. No actual
  tags, pushes, releases, or changes to dotask/filekeep were made.

### 2026-10-20 Use explicit version preparation and main-triggered releases

- Make shared build format, run strict Clippy, compile with private-path remapping,
  and scan executables while preserving the package version. Add shared/project
  run tasks that build once and execute Cargo's reported inspected binary with
  literal JSON-array arguments, preserving spaces, quotes, empty strings, Unicode
  and exit codes. Shared test builds once before workspace tests; the project
  wrapper also verifies shim integrity and Git whitespace.
- Make explicitly invoked update-version increment patch by default, with
  minor/major overrides and dry-run. Edit only the manifest/local lockfile version,
  preserving surrounding bytes and dependencies; do no build, Git operations or
  generated date-record writes. Keep ordinary compilation's human UTC stamp
  independent of the manually prepared SemVer version.
- Replace publish/tag-release tasks with the maintainer-owned test, version,
  review/commit, fast-forward main and push flow. Main pushes verify Windows/Linux
  and publish the same tested portable archives and self-extracting installers,
  checksums and metadata. Derive GitHub's internal tag from that committed version
  at the exact tested SHA; remove tag-push triggers and the second release build.
  Manual main runs remain artifact-only and build/collection jobs remain read-only.
- Refuse version tags that identify different source, authentication/network
  failures and incomplete drafts. Recognize an already published same-commit
  release through its existing tag and source metadata without replacing assets.
  Add the create-doinstall-installer project task for current native self-packaging,
  inspection and temporary-root acceptance. Document the new task/release flow
  and reusable task interfaces; retain Cargo-only bootstrap without Python.
- Verification: build.cmd verify and dotask test passed 154 Windows Rust tests, formatting,
  strict Clippy, shim integrity and whitespace. Disposable-project installed-dotask
  acceptance covered unchanged versions on build/run/test, explicit increments,
  previews/conflicts, argument boundaries, formatting, Clippy failure and scan
  failure before later operations. The actual publisher shell passed simulated
  new/existing/conflicting releases, network/draft/metadata/upload failures with
  fake gh. Actionlint 1.7.11 and Markdown file links passed. The project run task
  reported the current version; installer creation passed Windows ZIP and
  self-installer inspection, portable discovery and temporary-root install,
  consumer packaging and uninstall acceptance. No active installation or PATH
  changed. Cargo target/build directories remain C:/tmp/_rust/doinstall/target.
  No actual version increment, tag, push or release publication was performed.

### 2026-10-20 Fix publisher shell acceptance under Windows CI

- Give the publisher test's Bash process a fixture-owned native TMPDIR using
  forward slashes, so Bash and Windows jq resolve the same metadata files even
  when MSYS path conversion is disabled. Keep mktemp output within the fixture
  for automatic cleanup. Leave the production Ubuntu publisher unchanged.
- Explicitly disable MSYS conversion in Windows regression runs and use a
  fixture path containing spaces. Cover the same existing-release metadata
  lookup that failed in GitHub Actions, alongside all six other publisher cases.
- Verification: reproduced the original missing-metadata failure with disabled
  conversion and a POSIX-style TMPDIR; the corrected focused publisher tests pass
  under that same inherited environment. build.cmd verify passed all 154 Windows
  Rust tests, formatting, strict Clippy, shim integrity and Git whitespace checks
  with that environment retained. Cargo target/build outputs remain external at
  C:/tmp/_rust/doinstall/target. Preserve the prepared 0.1.1 version; no tags,
  pushes, releases, active installations or persistent PATH were changed.
