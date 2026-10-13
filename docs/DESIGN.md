# Doinstall design contracts

## Standalone installer design

This section records the agreed design and deliberately deferred features.
The implemented interface and native acceptance limits are documented in
[the installation contract](INSTALLATION.md) and README platform acceptance notes.

### Task and distribution boundaries

- Application build/publish steps remain project-specific. Callers compile and
  gather payloads, supply resolved YAML, invoke `doinstall-builder`, and optionally
  archive its output. Packaging and installation work with non-.NET payloads.
  Dotask's optional `_/doinstall/*` adapters are consumers of this ordinary CLI;
  their task management and project acceptance belong to dotask.
- Use a generic Rust installer, without an Inno Setup dependency or Windows
  Installed Apps integration. Retain the existing bundled Windows command shims;
  consumers do not rebuild them. Ordinary application installations support
  automated uninstall; lifecycle-managed services require an explicit cleanup workflow.
- Distribute a platform/architecture-specific executable, same-basename YAML,
  and a payload directory or ZIP. Optional pinned included tools run through
  [lifecycle actions](INCLUDED-TOOLS.md), with destination gates, owned-config
  snapshots, and failure journals. Server cleanup and automatic rollback are deferred.
  YAML carries application metadata, version,
  inputs, defaults, and installation instructions. Resolve payload paths relative
  to the YAML. No dotask or language toolchain is required on the destination.
- `doinstall-builder` assembles a package without installing. Its optional JSON
  result reports the absolute installer `FilePath`; callers choose whether to run
  it. The shipped installer accepts install, validate, and uninstall operations.

### Inputs and desktop integration

Install and uninstall are console-interactive by default. Installer YAML may
disable prompts; explicit `--interactive` / `--non-interactive` override it.
Redirected interactive execution fails with unattended-mode guidance. Package
creation and validation never prompt. Missing required information, invalid
values, or unresolved conflicts stop with an actionable error and nonzero status.
Named typed inputs own prompts and choices. Value precedence is CLI overrides,
explicit YAML values, environment-profile defaults, OS defaults, then common
defaults. Interactive answers may replace defaults. Environment profiles include
Linux desktop/distribution differences such as Omarchy.

Always create stable commands in the chosen app root. Additional commands are
optional, default No, and suggest that root's parent instead of consulting `BIN`.
Windows can append the selected command directory to user PATH, preserving
existing entries; an existing persistent PATH entry skips that question.
The dialog action button confirms its displayed choices; plain prompts confirm
a location summary. Cancellation returns 130; activation journals preserve the
existing recovery contract. YAML owns prompt
labels and defaults, while CLI values override individual questions.

The standalone installer reads only its packaged installer YAML. The creator
maps project settings to that document explicitly; it does not make the installer
depend on source-project configuration. Application name, optional copyright,
and description own the dialog title/banner. Interactive execution uses Cursive's
standard controls and appearance, with choices, operation progress, and summary
states with an internal default theme. Runtime `confirm-cancel` defaults to true and
asks before canceling choices or requesting cancellation during work. `--plain` retains indented line prompts
and separators sized to the visible console width; unattended output stays plain. Shortcut declarations grant
location permissions, while defaults set initial answers. The additional command
is always available today; its false default is a preference, not a permission.

Build systems own producer tasks, expected output selections, version policy,
and optional archives. Doinstall receives resolved application metadata, payload,
and nested runtime settings. It preserves that runtime structure in distributed
YAML and rejects source-only build steps and old root-level runtime fields.
Assembly runs in doinstall-builder on the build machine; no build command ships
inside the destination installer.

Launcher preferences may change during an update. Ownership comes from the
receipt's actual launcher inventory, not a required match against old preference
values. Lock previous and desired locations, verify owned files, refuse unowned
destinations, and remove only obsolete owned launchers. Pending activation retries
require the same package and requested destinations.

Shortcuts are available to console and GUI applications. YAML declares which
options are allowed and their defaults; CLI overrides and interactive choices
must respect those permissions. Support Windows Start menu and desktop shortcuts,
Linux application/desktop entries where supported, and macOS desktop aliases.
Terminal behavior, arguments, working directory, and icons need explicit metadata.
Desktop integration must be tested with an actual GUI consumer, including WinExe.

Shortcut names can reference the typed `shortcut-name` input, defaulting to the
application name, and must remain safe filesystem names after expansion. Windows
Start Menu links optionally use `name/name.lnk`, defaulting to flat placement;
desktop/local and Linux menu entries stay flat. Record only installer-created
menu folders and remove them only when empty. The standalone hello example
is a console application and declares no shortcuts.

Uninstall preserves settings by default. `--leave-settings` and
`--remove-settings` are mutually exclusive; removal is restricted to declared
application-owned settings, never arbitrary documents or unrelated user data.
An explicit unattended uninstall invocation does not require another confirmation
flag. The installed engine and records must suffice without the original payload.

### Installation layout and receipts

The application producer owns its version format. Build metadata optionally
supplies a frozen UTC `YYDDD-HHMM` stamp and Git revision; the installer never
substitutes its own version or the task runner's version. Named packages use
`<app-id>-<app-version>-<stamp>[-<git7>][-dirty]`; installed builds use that name
without the app prefix. Version strings remain verbatim in metadata, with unsafe
filesystem characters encoded only in directory components. Preserve full
content fingerprints to reject collisions and modifications without exposing
those fingerprints in metadata-based names. Reuse only identical packages.

Keep management files in a visible `installer/` directory beneath the application
root. The uniform payload layout is `app/<version>-<build-id>/`:

```text
pts/
  installer/
    installer.exe
    installer.yaml
    installation.yaml
  app/
    1.2.0-abc123/
      installer.yaml
      installation.yaml
      <application files>
    1.3.0-def456/
      installer.yaml
      installation.yaml
      <application files>
```

The executable has no `.exe` suffix on Unix. The root management receipt is
mutable installation state: identity, resolved locations, active and previously
active builds, owned shortcuts/commands, and installed build inventory. Each
build keeps an immutable receipt and a snapshot of its installer YAML. Uninstall
uses the corresponding installed configuration together with actual ownership
records, rather than recomputing locations from defaults or assuming the newest
configuration describes all older builds. Reserved metadata names must not
collide with payload files. The current YAML schema is defined in the public reference.

Always using version directories replaces the earlier fixed-location layout
proposal. Retaining multiple builds and an application's
ability to run from a version-specific path are separate concerns. Directory
layout alone cannot make incompatible application paths or data formats work.
Keep application settings in their normal locations; do not redirect XDG/AppData
settings through links into individual builds.

Convenience commands and optional shortcuts live in the application root:
for example `pts.lnk`, `pts.exe` plus `pts.shim`, or a Unix command symlink and
desktop launcher/alias. There are independent local and external
launchers targeting the active application build directly. External shortcuts
are refreshed on installation, without chaining through local ones.
Both sets require ownership tracking, collision checks, and uninstall handling.

Encourage versioned installations. `--prune-old-versions`, a YAML default, or an
interactive choice enables pruning after successful installation and activation.
Keep the new active build and the last active build, not the next lower version
number. An identical reinstall must not shift that history. Preserve modified or
otherwise unsafe-to-remove builds and report them. Without pruning, retain older
builds. Installations from previous formats are unsupported; do not import them
or adopt their directories and launchers.

### Rollback is deferred

Do not implement `--rollback` or `--set-version` in the initial feature. Retaining
a previous build does not promise that it can safely be reactivated. The reasons
to defer are:

- Newer applications may migrate settings, databases, or documents incompatibly;
  changing executable paths does not undo those migrations.
- Running applications keep using their current build; switching launchers does
  not switch existing processes.
- Builds can differ in entry points, commands, icons, and shortcut definitions.
- Updating several launchers can be interrupted, leaving mixed activation state;
  recovery and ownership checks require a separate design.
- Retained builds can be missing or modified and must be validated before use.

Users may invoke an older installer, but that is a new installation attempt, not
a guaranteed data downgrade or automatic rollback. Automated failure recovery is
also distinct from a user-requested version switch. Deferring version switching
preserves the installer's existing forward activation recovery behavior.

## Verification and release gaps

Windows x64 tests exercise temporary installation, updates, ownership conflicts,
activation recovery, shortcuts, and uninstall. Native Linux/macOS and Windows
ARM64 acceptance remains pending; a CI job definition is not evidence of a run.
The standalone example supplies a reproducible smoke-test flow for each host.

Cargo.lock is checked in and product builds use `--locked`. CI currently installs
floating Rust `stable`; there is no pinned release toolchain or automated release
publication. Frozen application build metadata makes package names repeatable,
not a claim of byte-identical compiler output. Shim source changes require LLVM
rebuilds and native acceptance; the shim rebuild CI job verifies generated hashes
but does not currently compare rebuilt assets against committed bytes.

## Themed Cursive presentation

Cursive uses only its Crossterm backend, compiled into the executable without an
ncurses or language-runtime dependency. Standard controls own editing, selection,
checkboxes, focus, and Tab navigation. The existing resolver owns value precedence
and validation; the form does not implement a competing installation contract.
The engine reports operation phases and messages through a worker-scoped observer.
A worker thread keeps the event loop responsive without adding an async runtime.

Cancellation is cooperative before destination mutation. After the first write,
installation/removal completes or preserves existing retry records on error.
Late requests are reported honestly as completion. Backend cleanup precedes final
console output; unattended/plain paths retain their existing behavior. The dialog
scrolls at small sizes and preserves its fields behind a minimum-size message.
Runtime `confirm-cancel` defaults to true. Confirmation is modal, defaults to No,
and does not signal the worker before Yes. Keyboard and external Ctrl+C requests
use the same policy. Completing work removes any pending confirmation and shows
the result; closing a summary does not ask.

`runtime.theme` is a strict theme identifier; only `default` is accepted. Colors,
padding, sizing, positioning and layout are internal rules, not package settings.
External theme YAML loading remains deferred. Optional input `group-index` is
presentation metadata: stable sorting groups related controls without changing
resolution or plain-mode prompt order. Both additions omit unused defaults when
serialized to preserve package content identity.

The form wrapper paints space-filled edges and a header/footer while Cursive's
Dialog owns geometry, focus, buttons and hit testing. A separate body palette
allows padded raw button labels to have normal/focused background fills without
coloring ordinary text as buttons. Validation, cancellation, warnings and errors
use the same presentation. Checkbox rows add a focus marker, full-label highlight
and row clicking while retaining the stock checkbox state and enablement checks.

TextArea supplies soft wrapping and Unicode cursor editing for single values.
The wrapper refuses typed newlines, preserves Tab navigation, and limits height
to four rows with cursor-following scrolling. Editing invalidates layout so fields
can grow or shrink with their content. Progress remains phase text, and summary
text wraps inside the scrolling body. No spinner, progress bar or percent is added.
