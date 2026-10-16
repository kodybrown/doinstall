# Installer configuration and ownership

Doinstall's builder accepts resolved application metadata, payload location, and
nested `runtime` settings, plus optional producer-only `packaging` names.
Producer tasks, gathers, application payload creation, and version
policy are responsibilities of the chosen build system, not the destination
installer. Doinstall does not parse dotask settings or task files. It can launch
pinned included tools through [runtime lifecycle actions](INCLUDED-TOOLS.md).

Run the build-machine tool with explicit installer, config, and output paths:

```sh
doinstall-builder --installer /tools/doinstall --config installer.yaml --output /delivery/example-1.0
```

Windows tools use `.exe`. Builder paths resolve from its working directory; the
payload resolves from the configuration file. Explicit `--output` directories
must be fresh; keep the builder out of the resulting distribution. Distribute the
complete installer directory. Run the packaged executable with
`--non-interactive --install-dir ABSOLUTE_PATH` for unattended
installation, supplying the YAML's required inputs. The default installer name
is `<id>-setup-v<version>[.exe]`, with matching `<id>-setup-v<version>.yaml`.
Interactive behavior and shortcuts are controlled by `runtime`.

## Packaging names

The source YAML may include builder-only settings:

```yaml
packaging:
  installer-name: dotask-setup-v1.23.2610.1413.exe
  launcher-name: dotask-setup-v1.23.2610.1413-se.exe
```

These are literal portable filenames, not paths or version templates. Unknown
settings, reserved names, collisions, and Windows names without `.exe` fail.
The defaults derive from `application.id` and the resolved application version;
unsafe version bytes use the same percent encoding as build directory names.
Unix defaults omit `.exe`. The installer YAML name is derived automatically:
remove only a final `.exe` (case-insensitive), then append `.yaml`. Periods in
Unix versioned executable names remain intact. The runtime uses this exact name
beside its executable; `--config FILE` is the only override, with no alternative
name search or fallback.

The builder removes `packaging` from distributed YAML. The destination runtime
does not accept producer settings. Installed management files retain their
internal `installer[.exe]` and `installer.yaml` names, independent of delivery
names, so receipts, recovery, and uninstall keep their ownership contract.
There are no compatibility loaders, conversions, or migrations.

## Self-extracting launchers

### Build-machine recipe for people and AI assistants

1. Obtain `doinstall-builder[.exe]` for the build machine, and matching
   `doinstall[.exe]` / `doinstall-launcher[.exe]` tools for the destination OS and
   architecture, from the same doinstall build. The launcher is a template, not
   a generic ZIP extractor. Do not substitute a command shim for either target
   executable. Prebuilt tools need no Rust SDK; building these tools from source
   is a separate developer step (`build --stage-dir PATH`).
2. Build/publish the application into a payload directory. Write resolved YAML
   with application identity/version, target platform/architecture, payload path,
   and runtime settings. The native builder does not run build steps. For example,
   save this as `installer.yaml` beside a `published/` directory containing
   `example.exe`:

   ```yaml
   schema: 1
   application: {id: example, name: Example App, version: 1.2.0}
   platform: windows
   architecture: x64
   payload: ./published
   runtime:
     interactive: true
     commands:
       - {name: example, executable: example.exe}
     defaults:
       windows: {add-to-path: false}
   ```

3. Create the result file's parent, then run the builder. From that project
   directory in PowerShell, with tools under `C:/Tools/doinstall`:

   ```powershell
   New-Item -ItemType Directory -Force ./artifacts | Out-Null
   & C:/Tools/doinstall/doinstall-builder.exe --installer C:/Tools/doinstall/doinstall.exe --launcher-template C:/Tools/doinstall/doinstall-launcher.exe --config ./installer.yaml --output ./artifacts/example-1.2.0 --self-extracting --result-file ./artifacts/example-result.json
   $launcher = (Get-Content ./artifacts/example-result.json -Raw | ConvertFrom-Json).FilePath
   & $launcher --extract-to ./artifacts/validation-source -- --validate
   ```

   The output directory must be fresh. Use the returned `FilePath`; do not
   reconstruct it from names. The result file must remain outside the package.
   Validation extracts and retains sources but does not install the application.
   On Linux/macOS use host/target tools without `.exe`, appropriate YAML target
   values, and executable payload names. The full builder invocation is below.
4. Distribute only `artifacts/example-1.2.0/example-setup-v1.2.0-se.exe`.
   Running it opens the installer in the same console. For automation, pass
   `--non-interactive` and explicit temporary installation/command roots when
   testing. Use `--extract-to PATH` for a chosen new/empty extraction directory;
   extracted sources remain after the installer exits. See the details below
   for signing, cross-platform packaging, and retained sources.

Set optional literal delivery filenames under `packaging` in the source YAML;
the builder derives the matching installer YAML name automatically. Keep
application build steps and dotask settings outside native resolved YAML.
If using dotask, its separately installed shared adapters and source build-step
contract are documented in
[dotask's installer guide](https://github.com/kodybrown/dotask/blob/develop/docs/INSTALLATION.md#self-extracting-build-and-gather-workflow).

### Container, extraction, and process contract

```sh
doinstall-builder --installer /tools/doinstall --config installer.yaml --output /delivery/example-1.0 --self-extracting --launcher-template /tools/doinstall-launcher --result-file /delivery/result.json
```

Windows tools use `.exe`. `--output` remains a fresh output **directory**;
self-extracting mode publishes only the completed launcher inside it. Directory
mode publishes the installer, matching YAML, and payload. `--output-parent`
derives a directory from build metadata and reuses only matching verified
contents. `--result-file` reports the absolute runnable artifact in either mode.
Do not distribute the builder or the unassembled launcher template.

The builder and launcher template are ordinary compiled Rust tools. Consumers
need no Rust SDK, shell packaging script, or installed ZIP utility to assemble
or execute a package. Supply the installer and launcher template for the
package's target platform/architecture. The builder's host can differ from the
target; it validates target binaries without executing them. Native builders
can be built for Windows, Linux, and macOS, x64 and ARM64. Windows distribution
tools should be built with the repository launcher to use the static CRT.

The container is a precompiled native executable followed by a Deflate ZIP,
strict schema-1 JSON manifest, and versioned footer. Unsupported formats fail
directly. SHA-256 checks detect archive/file corruption; they are not publisher
authentication. Extraction refuses traversal, links, aliases, duplicate entries,
unknown files, and file/directory collisions before starting the installer.
Target Unix executable permissions are carried in the archive.

```sh
./example-setup-v1.0-se --extract-to /chosen/new-directory -- --non-interactive
```

`--extract-to PATH` selects the exact extraction directory, relative to the
caller's working directory when not absolute. It must be new or empty and must
not traverse links/junctions. Without it, each execution creates a fresh
directory under the OS temporary root, prefixed with the launcher's filename
stem and a random suffix. System temporary-root aliases are resolved before
creating that directory. The launcher prints its source directory to stderr.
All extracted files are intentionally retained after success, failure, or
cancellation, including incomplete extraction on failure. There is no cleanup
or sweep of earlier runs. OS temporary cleanup is not guaranteed; use an
explicit persistent path for long-term retention. Rerun the extracted installer
directly to reuse that source directory; a new launcher run extracts afresh.

`--launcher-help` describes extraction without extracting anything. Other
arguments are passed to the installer; `--` ends launcher option parsing. Both
processes use the same console. The launcher inherits standard handles,
environment, and working directory, with no I/O capture or forwarding. Windows
waits quietly and returns the installer status, keeping itself alive during
Ctrl+C cancellation so the installer can restore the terminal. Linux/macOS use
`exec` to replace the launcher with the installer. No new terminal is opened.
Existing terminal requirements for interactive mode still apply.

Sign the completed launcher after packaging, not the input template. Input
template signatures are removed because they cover only the original bytes.
The footer reader accounts for PE certificate and Mach-O signature trailers.
Native macOS packaging extends `__LINKEDIT` and uses the OS-provided `codesign`
for ad-hoc signing. When packaging a macOS target on another OS, sign the final
file on macOS before running it (`codesign --force --sign - FILE`); trusted
distribution signing/notarization remains the producer's responsibility.
When copying a Unix launcher from Windows, set its executable bit on Unix.

## Installer YAML schema 1

This example packages an already-built Windows application. Use `linux` or `macos`
and appropriate executable paths on those systems. Architectures are `x64` and
`arm64`. Paths inside the payload use `/` and must not escape the payload.

```yaml
schema: 1
application:
  id: example
  name: Example App
  version: 1.2.0
  build:
    stamp: 26279-0612       # UTC YYDDD-HHMM; supplied by the application build
    commit: abcdef71234567890123456789012345678901234  # optional
    dirty: false
  author: Example Author
  copyright: Copyright (C) 2026 Example Author
  description: A desktop application.
platform: windows
architecture: x64
payload: ./published
runtime:
  theme: default           # optional; currently the only supported theme
  interactive: true
  confirm-cancel: true      # dialog cancellation confirmation; defaults to true
  commands:
    - name: example
      executable: example.exe
  shortcuts:
    - name: '${shortcut-name}'
      executable: example.exe
      arguments: []
      terminal: false
      desktop: true
      start-menu: true
      local: true
  inputs:
    channel:
      type: choice
      choices: [stable, preview]
      required: true
      default: stable
      prompt: Choose a channel
      group-index: 3       # optional visual group; does not affect value resolution
  defaults:
    common:
      additional-command: false
      confirm-install: true
      desktop-shortcuts: false
      local-shortcuts: true
    windows:
      add-to-path: true
      start-menu-shortcuts: true
      start-menu-nested: false
      shortcut-name: Example App
  profiles:
    omarchy:
      values:
        desktop-shortcuts: false
  values: {}
  settings:
    - '${local-app-data}/Example/settings'
```

The runtime finds matching-name YAML beside itself, independent of working directory
(only `.exe` is stripped before appending `.yaml`).
`--config FILE` overrides that path. Payload paths resolve relative to the YAML;
packaging rewrites the payload reference to its packaged directory or ZIP.
ZIP traversal, duplicate paths, and links are rejected. Links inside directory
payloads are also rejected. Executable permissions are preserved on Unix.
`installer.yaml` and `installation.yaml` are reserved at each payload's root.

Inputs support `string`, `path`, `boolean`, and `choice`, with `required`, `prompt`,
`default`, and `choices`. Optional `group-index` is a nonnegative integer for
interactive presentation only. Equal indices appear together, groups sort in
ascending order, and controls retain their existing order within each group.
Built-in defaults are 0 (installation location), 1 (command options), and 2 (shortcuts).
Application inputs default to 3. Groups are separated by blank spacing without
generated headings. An application input can join a built-in group by using its
index. Grouping never changes input precedence, validation, dependencies, or the
order of plain-mode prompts. Values are resolved in this order, highest first:

1. CLI overrides (`--set NAME=VALUE`).
2. Explicit YAML `values`.
3. Selected profile `values`.
4. OS `defaults`.
5. `defaults.common`, input defaults, then built-in location defaults.

Profiles are selected by `--profile NAME`, or optional `detect` strings matching
`XDG_CURRENT_DESKTOP` components. Multiple matches fail and require an explicit
profile. Omarchy can be selected explicitly; distribution detection is not inferred
from a desktop identifier. Unknown fields/inputs and invalid types/choices fail.
Templates support `${home}`, `${local-app-data}`, `${config-home}`, and named inputs.
Unresolved references fail. Built-in path inputs must resolve to absolute paths.

## Unattended and interactive execution

The installer is console-only. Install and uninstall default to prompting;
`interactive: false` in `installer.yaml` selects unattended execution.
`--interactive` and `--non-interactive` override that setting and are mutually
exclusive. Interactive execution requires a terminal; redirected execution fails
with guidance to use `--non-interactive`, rather than silently installing.
Package creation, help, and `--validate` never prompt. Missing required values,
invalid configuration, and ownership conflicts stop with a diagnostic and nonzero
exit code. Interactive answers replace defaults; CLI-supplied inputs are read-only
in the dialog and skip their individual plain-mode prompts. Automation does not
require a second confirmation flag.

Interactive installation uses a themed Cursive form with three states:
installation choices, operation progress, and a summary with a Close button.
Uninstall uses the same flow, offering removal of declared settings unless a CLI
settings flag already supplies the policy. The title, copyright, and description
come from application metadata; the choices title also includes the version.
`runtime.theme` selects a theme by identifier, defaulting to `default`. Only this
internal theme is supported now. Custom `theme.yaml` files are deferred, and
unknown identifiers or file paths are rejected. The installer YAML cannot specify
individual colors, padding, dimensions, position, layout, or UI behavior settings.
The separate `confirm-cancel` runtime policy described below remains supported.
Default theme selection and omitted group indices are omitted from serialized
packages, so unused presentation metadata does not change package snapshots.

The default theme uses a dark backdrop, contrasting form surface, colored space
cells for the side edges, half-block title caps, and a one-column/half-row offset
shadow. Popup text has one blank surface row above and below it. Headers, footers, normal/focused buttons,
fields, warnings, errors, and cancellation/validation dialogs share that theme.
Buttons use padded labels without angle brackets. Focused checkboxes highlight
the complete row without a marker or indentation; clicking the row toggles the checkbox.
Enabled checkbox labels are white, with a darker focused row for contrast.
Long checkbox labels and choice options wrap within the form; continuation rows
retain the same focus colors and mouse target. Choices remain single values,
with arrow keys navigating options rather than individual wrapped lines.
Descriptions, PATH explanations, and field captions use subdued gray; disabled
controls remain muted. The built-in installation caption is `Install to:`;
application-defined prompts and plain-mode wording remain authoritative.
Choices show `Tab:move  Space:toggle Esc:cancel` on the button row; narrower
terminals use shorter hints on that same row without covering the buttons.
Tab/Shift+Tab navigation remains available even when the compact hint omits it.
Windows places affirmative buttons first (Install/Cancel or Yes/No); Linux/macOS
places the negative button first (Cancel/Install or No/Yes).

Tab/Shift+Tab moves between enabled controls; Space toggles checkboxes. CLI inputs
are shown read-only. Dependent fields are disabled until their option is enabled,
retaining their values. Invalid inputs keep the choices screen open with a
validation diagnostic. The dialog grows horizontally with the console, leaving
a small outer margin; text fields use the additional width. Long input values
soft-wrap without inserting newlines into their stored values. Editors grow to
at most four rows, then scroll to keep the cursor visible. Enter does not add a
newline; Tab/Shift+Tab continues navigating controls. Unicode content and cursor
positions survive resizing. Height follows the
content up to the available space. Configuration and result bodies scroll when
space is limited. Below the minimum usable area (approximately 50 columns by 14 rows,
including dialog margins), a resize message replaces the view without losing
field values. The notice uses explicit contrasting text on the backdrop, including
when only the width or only the height is too small. Resizing during work does
not suspend filesystem operations.
Progress names the current operation, with no spinner, bar, or percentage. Summary
paths and errors wrap, and Close restores the terminal and prints operation
messages in scrollback. Cancellation returns 130, errors return 1, success 0.

`--plain` selects the original line-oriented prompts and summaries. This mode
prints the application banner, Installation Settings, and Installation Summary
(or corresponding Uninstallation sections). Separators follow visible console
width, including Unicode widths and narrow windows; unavailable width information
falls back to 80 columns. Unattended output remains plain.

`runtime.confirm-cancel` is a boolean that defaults to true. In the interactive
install/uninstall dialog, Cancel, Escape, or Ctrl+C asks for confirmation before
canceling choices or requesting a stop during work. The popup title is `Confirm`.
No is the default button regardless of its platform-specific position;
Enter, No, or Escape in that confirmation resumes the dialog without losing
values. Set `confirm-cancel: false` to request cancellation immediately. Closing
a completed summary never asks for cancellation confirmation. If work finishes
while confirmation is open, its summary replaces that confirmation automatically.
This option does not introduce prompts in unattended execution or change the plain-mode UI
Ctrl+C behavior. It is a runtime policy, not a `values` input or CLI flag.

After confirmation, cancellation stops at the next safe pre-write checkpoint. Once destination changes begin, the operation finishes rather than
abandoning a partially owned installation; failures retain the existing recovery
records. A late cancellation is reported as a completed operation in the summary,
not as an undo or rollback. Plain-mode Ctrl+C retains immediate interruption and
its existing journal/retry behavior. During an included tool, plain cancellation
first stops the owned process tree and saves its workflow recovery state.

The install prompts ask for the complete application directory, an optional
additional command (default No), its directory when enabled, and on Windows
whether to add the selected command directory to the user PATH (default Yes).
The additional directory is derived from the chosen install directory's parent,
including interactive changes, unless YAML or `--bin-dir` supplies another path.
If no additional command is selected, the application directory is the PATH
candidate. A directory already on persistent user/system PATH produces an
informational message and skips the registry write (and the plain-mode PATH
question). In the form, its PATH checkbox is checked and disabled, with an
explanation that no PATH change is needed. This checkmark represents existing
membership, not a new request to modify PATH. Changing to a directory outside
PATH restores the previous choice unless a CLI override locks it. A CLI-locked
PATH option explains that restriction as well; the hello preview deliberately
uses `--set add-to-path=false` to prevent PATH writes.
The dialog's Install button confirms the selected settings. In plain
mode, a final location summary precedes confirmation; boolean answers accept Y/N,
yes/no, and true/false, and Enter accepts the shown default. Ctrl+C, end of input, or
declining plain-mode confirmation prints `Canceled` and exits 130. Cancellation
before confirmation creates no installation. Interrupted activation retains its
recovery journal.

```sh
./installer --non-interactive --install-dir /tmp/example/apps/example --bin-dir /tmp/example/bin
./installer --interactive
./installer --interactive --plain
./installer --non-interactive --set desktop-shortcuts=false --prune-old-versions
./installer --validate
./installer uninstall --install-dir /tmp/example/apps/example
```

Built-in inputs are `install-dir`, `bin-dir`, `desktop-dir`, `start-menu-dir`,
`desktop-shortcuts`, `start-menu-shortcuts`, `local-shortcuts`, and
`prune-old-versions`, plus `additional-command`, `add-to-path`, and
`confirm-install`, `shortcut-name`, and `start-menu-nested`. These defaults and their prompt labels can be overridden using
`defaults`, `values`, and `inputs` in installer YAML. `confirm-install` controls
the plain-mode confirmation default; the dialog always requires its action button.
Boolean flags enable the corresponding option; use
`--set NAME=false` to disable it. Shortcut and pruning defaults are false unless
YAML changes them. `--validate` checks package/configuration without installing.
Failures return 1, cancellation returns 130, and success returns 0. Detailed errors identify failed operations;
output can be captured by the invoking automation. There is no separate log-file
or dry-run installation-plan interface yet.

Build systems invoke the packaged executable with ordinary argument tokens:

```sh
./installer --non-interactive --set add-to-path=false --install-dir /tmp/example/apps/example
```

Specify one action and each input only once; contradictory actions and duplicate
input overrides fail. Options requiring a value reject a following `--option`
as a missing value. Use `--set NAME=VALUE` for input strings beginning with `--`.

## Locations, launchers, and version retention

### Package and installed build names

Versions belong to the application being packaged, never the installer engine
or the build tool running the builder. `application.version` is an arbitrary
nonempty version string: SemVer, calendar versions, and custom labels are
supported. Common filename-safe characters, including SemVer `+` metadata, stay
readable; unsafe characters and literal percent signs are percent-encoded only
in directory names. The original version remains in metadata and display output.
Encoded version/build components must fit the filesystem-safe name limits.

Supply `application.build.stamp` as UTC `YYDDD-HHMM` to match the application's
embedded build information. Git `commit` is optional; when supplied it is stored
in full and displayed in names using its first seven characters. `dirty: true`
adds `-dirty` and requires a commit. When package creation has no build metadata,
the engine captures a packaging minute without a Git revision and saves it in
the package. Installation always uses that saved stamp. Direct installation of
an un-packaged config without build metadata retains a content-addressed identity.

`doinstall-builder --output-parent DIRECTORY` (with `--installer` and `--config`)
creates a directory named
`<app-id>-<app-version>-<stamp>[-<git7>][-dirty]`, with the engine, YAML, and payload
directly inside. The installed build under `app/` uses the same name without the
`<app-id>-` prefix. `--output DIRECTORY` remains an explicit complete package-path
override. `--result-file FILE` writes the absolute installer `FilePath` as JSON;
callers read that result instead of parsing console output. The result file's
parent must exist, and the file must be outside the completed package and must
not overlap the source config, installer executable, or payload. Invalid result
paths fail before package publication. A later I/O failure writing the report
leaves the completed package intact; the diagnostic returns failure.

Identical named packages can be reused. Matching names with different payload or
configuration are rejected without overwriting the package or installed build.
The internal full-content fingerprint remains an integrity check; no fingerprint
is appended to build-metadata-based names. Previously owned build directories
retain their names and receipts; no directory renaming or adoption is performed.

Default application roots are `%LOCALAPPDATA%/Programs/<id>` on Windows and
`~/.local/lib/<id>` on Linux/macOS. `--install-dir` selects the complete application
root, not its parent. A stable command is always created inside that root. An
additional command is optional (`additional-command: false` by default); its
suggested directory is the application root's parent. `BIN` is not consulted.
`--bin-dir` enables the additional command unless `--set additional-command=false`
explicitly disables it. Its location must be outside the application root;
an ancestor such as `%LOCALAPPDATA%/Programs` is permitted. Updates may enable,
disable, or move the additional command. The receipt's launcher inventory owns
the previous files independently of saved preference values. The installer locks
the previous and requested directories, verifies existing owned files, creates
the selected launchers, and removes only obsolete owned launchers. An unowned new
destination or modified old launcher stops the update before activation. A pending
activation must be retried with the same package and launcher destinations.

On Windows, `add-to-path` defaults to true and applies to the additional command
directory when enabled, or the application directory otherwise. The installer
appends only a missing directory to persistent **user** PATH, preserves existing
entries and the registry string type, and notifies Windows of the change. Existing
terminals keep their environment; reopen them to use the new command. Uninstall
removes only owned shims/sidecars and retains PATH entries, including shared parent
directories that may contain other applications' commands. Automatic PATH changes
are unavailable on Linux/macOS; add the selected directory to your shell's PATH.
Explicit directory aliases are resolved before operating; managed child links
and junctions are rejected.

```text
example/
  installer/
    installer.exe
    installer.yaml
    installation.yaml
  app/
    1.2.0-<build-id>/
      installer.yaml
      installation.yaml
      <application files>
  example.exe
  example.shim
  Example App.lnk          # when local shortcuts are enabled
```

Unix root commands are symlinks. Windows root commands use the existing native
shim plus a `.shim` sidecar. These provide a stable launch path while the real
executable remains in its build directory. External commands/shortcuts point
directly to that same build, not through the root launcher. Installation refreshes
both sets. No fixed-location payload mode is implemented.

Each build has a configuration snapshot and immutable inventory receipt. The
management receipt records actual locations, active/previous builds, launcher
ownership, settings locations, and hashes. Build identity includes payload files,
Unix permissions, and package configuration; timestamps are not identity.
Repeating an identical install does not change the previously active build.

`--prune-old-versions` prunes after successful activation, keeping the new active
build and the last active build. Modified or unsafe builds are retained with a
diagnostic. Older builds otherwise remain. Rollback and `--set-version` are deferred:
retention does not imply data/settings downgrade compatibility.

## Shortcuts

Console and GUI applications can both declare shortcuts. Each shortcut's
`desktop`, `start-menu`, and `local` booleans permit those locations; corresponding
input values select whether to create them. Requesting a location with no allowed
shortcut fails. Optional `icon` and `working-directory` are payload-relative.

Declare `name: '${shortcut-name}'` to offer a configurable shortcut name, which
defaults to `application.name`. Static names remain fixed; a package may declare
multiple distinct shortcuts. `--shortcut-name NAME` or `--set shortcut-name=NAME`
overrides the named input. Names omit the extension and may contain spaces or
Unicode. Empty/oversized names, control characters, path separators, invalid
Windows characters, reserved device names, and trailing dots/spaces are rejected.
An invalid interactive name reports the error and asks again. Template-expanded
names are validated before a shortcut is staged or installed.

On Windows, enabling a Start Menu shortcut prompts for its configurable name and
whether to nest it in a folder of the same name (default No). Set
`start-menu-nested: true` in Windows defaults, use `--start-menu-nested`, or pass
`--set start-menu-nested=true` to select `name/name.lnk`; otherwise the location
is `name.lnk`. The base defaults to the current user's Start Menu Programs folder.
Desktop/local shortcuts remain flat. Updates may rename shortcuts or change
their nesting; obsolete owned links are removed. Receipts record menu folders
created by the installer, which are removed only when empty. Preexisting folders
and folders containing unrelated files are preserved.

Linux application-menu `.desktop` entries remain flat. Windows nesting is rejected
on other platforms, since a desktop-file subdirectory does not itself define a
visible Linux submenu. Linux/macOS acceptance remains deferred. Doinstall's example
console package declares no shortcuts, so it offers no Start Menu question or
entry; the shortcut options are for applications packaged by this engine.

Windows uses native `.lnk` files and application entry points determine console
behavior. Linux uses `.desktop` files with `Terminal` metadata; desktop visibility
and trust still depend on the desktop environment. Defaults use `~/Desktop` and
`$XDG_DATA_HOME/applications` (falling back to `~/.local/share/applications`);
localized/custom desktop locations can use `desktop-dir`.
macOS GUI aliases target app bundles and use their launch defaults/icon; aliases
with custom arguments, working directories, or icons are rejected. Terminal
shortcuts use `.command` files with explicit arguments and working directories.
macOS has no Start menu: requesting it fails. Desktop aliases may require Finder
automation permission; native macOS acceptance remains pending.

## Uninstall, settings, and ownership

Run the retained executable without rebuilding:

```sh
/path/to/example/installer/installer uninstall
/path/to/example/installer/installer uninstall --install-dir /path/to/example
```

On Windows, invoke `installer.exe`. Uninstall uses installed build snapshots and
ownership receipts; the original payload/package is unnecessary. Settings are
preserved by default. `--leave-settings` makes that explicit; `--remove-settings`
removes declared application-owned settings paths. They are mutually exclusive.
The runtime accepts `--remove-settings` and `--non-interactive`; otherwise
the retained installer's YAML controls prompt mode.
Application documents and unrelated files must never be declared as settings.
Settings remain in their ordinary locations, not linked into version directories.

Unowned destinations are never adopted. Modified owned launchers/builds block
uninstall before deletion; resolve the diagnostic rather than deleting receipts.
Unrecorded application files are preserved and reported. On Windows, close running
applications before uninstall; locked payloads fail preflight. The engine does not
force-close applications or manage services. A running Windows uninstaller may
move itself to a printed temporary directory, which can be deleted after it exits.

Operations are locked by installation and command directory. Activation keeps a
`pending.yaml` record for forward completion: retry an interrupted installation
with the same package and locations. Unknown modifications still fail. Uninstall
records progress for retry after partial removal. This is not user-requested
rollback, nor a guarantee that arbitrary failures restore the previous state.

Only the current YAML installation format is supported. There is no import,
migration, or backward compatibility with older installation formats. A directory
without a valid YAML receipt is never adopted, even if it contains dotask files.
Use a fresh installation root and unused command destinations, or manually remove
the previous installation and its launchers before using those locations again.
The new installer does not uninstall installations created by the previous engine.
There is no Windows Installed Apps registration or Inno Setup dependency.
