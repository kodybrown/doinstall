# doinstall

A lightweight, cross-platform installer built in Rust.

Doinstall assembles application files and resolved YAML into an independent
installer directory or one ZIP self-extracting launcher. Its destination runtime installs immutable versioned builds,
creates owned commands/shortcuts, and retains an uninstaller. It works without
dotask, .NET, or a language SDK on the destination machine; applications may have
their own runtime requirements.

## Install doinstall

Versioned GitHub releases provide a self-extracting installer for people and a
portable tools archive for CI on Windows x64 and Ubuntu 22.04 x64 / glibc 2.35.
See [releases and pinned downloads](docs/RELEASES.md) for exact names, checksums,
tool layout, compatibility limits, and the pending first-release procedure.
Keep the matching builder, runtime, and launcher template together; consumers
need no Rust SDK or doinstall source checkout.

Build doinstall's own self-extracting installer from this source checkout:

```powershell
.\build.cmd package
```

Run the `doinstall-setup-v<version>-se.exe` printed by that command. It installs
the tools and their command shims using the same ownership, update, and uninstall
flow as any other application. No manual directory/shim setup is needed.
Linux uses `./build.sh package`; macOS and ARM64 acceptance are deferred.
See [tool installation](docs/INSTALLATION.md#install-doinstall-tools) for paths,
uninstall, and tool discovery from dotask or another build system.

## Build from source

The pinned Rust 1.95.0 toolchain, Cargo, rustfmt, clippy, and a host linker are
required. Rustup reads `rust-toolchain.toml`; release scripting also uses Python 3.10+.

```powershell
.\build.cmd build
.\build.cmd verify
```

Linux/macOS use `./build.sh build` and `./build.sh verify`. With no arguments the
launcher lists development commands. Outputs remain external; locate them with
`cargo metadata --format-version 1 --no-deps --locked` (`target_directory`).
`doinstall[.exe]`, `doinstall-builder[.exe]`, and the precompiled
`doinstall-launcher[.exe]` template are under `release/` there.
`build --stage-dir PATH` stages those tools and the optional `doinstall-server`
helper into a selected directory. The helper supports the reusable
[Debian/Ubuntu web stack recipe](docs/DEBIAN-WEB-STACK.md); the engine does not
require it.

## Create an installer

```sh
doinstall-builder --installer /tools/doinstall --config /project/installer.yaml --output /delivery/example-1.0
```

On Windows pass the corresponding `.exe` paths. The builder is a build-machine
tool; distribute only the resulting directory (by default,
`example-setup-v1.0[.exe]`, matching `.yaml`, and payload). The installer never runs producer tasks or reads
.dotasks.yaml. It can launch pinned tools bundled in its payload through
[runtime lifecycle actions](docs/INCLUDED-TOOLS.md). See
[the configuration and ownership contract](docs/INSTALLATION.md).

Try the [runnable standalone hello example](examples/hello/README.md), which
assembles, validates, installs, runs, and uninstalls entirely under temporary roots.

Applications can select any build system. Integration task files belong to that
build system, which invokes these ordinary command-line tools. Install the tools
separately; doinstall does not discover projects, install build-system adapters,
or manage their configuration and updates. Keep the matching runtime and builder
from the same build together on the build machine.

Add `--self-extracting` to publish one launcher, named
`example-setup-v1.0-se[.exe]` by default. The builder uses its adjacent precompiled
launcher template, or an explicit `--launcher-template FILE` for the target OS
and architecture. Assembling packages requires no Rust SDK or installed archiver.
The launcher extracts into a fresh retained temporary directory (or
`--extract-to PATH`) and starts the installer in the same console with direct
input/output. Windows waits and returns the installer's status; Linux/macOS
replace the launcher process with the installer. See the
[step-by-step build-machine recipe](docs/INSTALLATION.md#build-machine-recipe-for-people-and-ai-assistants)
and [self-extracting contract](docs/INSTALLATION.md#self-extracting-launchers).

## Interactive installer

Install and uninstall use a themed Cursive form with colored surfaces, a shadow,
filled buttons, grouped options, and keyboard navigation. Focused checkboxes
highlight their whole row and align with the fields; long input values soft-wrap. Choices,
text-only progress, and wrapped summaries occupy separate screens. Cancel/Escape/Ctrl+C confirms by default;
`runtime.confirm-cancel: false` skips the confirmation. `--plain` uses the original line prompts;
`--non-interactive` keeps automation output plain. `runtime.theme: default` selects
the internal theme; colors and layout are not package settings.
Try the [temporary Windows UI preview](examples/hello/README.md#interactive-windows-preview).

## Development and acceptance

`verify` runs formatting, Rust tests, clippy, shim hashes, and Git whitespace.
Tests use disposable install/command/shortcut directories and a disposable
registry key; they do not modify an active installation or the user's PATH.
The inherited Windows x64 runtime coverage is retained. The user also reported
a successful Linux x64 interactive preview in Debian WSL using a native debug
build. The combined UI/included-tools branch also passed 79 Rust tests in Debian
WSL using a native debug build. macOS and Windows
ARM64 acceptance remain pending.

The web-stack work also passed release verification on Windows x64 (100 tests)
and native Ubuntu x64 (88 tests, using a temporary Linux clone for Git metadata
compatibility). See the [web stack contract](docs/DEBIAN-WEB-STACK.md) for server
recipe acceptance and remaining certificate/ARM64 limits.

The self-extracting launcher passed full release verification on Windows x64
(120 tests) and Ubuntu x64 in WSL (102 tests). Native console previews passed
display, cancellation, terminal restoration, and retained-source checks on both
systems. A Linux launcher assembled by the Windows builder also passed Linux
install/validate/launch/uninstall acceptance. Native macOS and ARM64 execution,
distribution signing/notarization, and remote CI remain unverified.

Release automation additionally passed local portable-bundle and self-hosted
installer acceptance on Windows x64 and native Ubuntu 22.04.5 x64 in Docker,
including downstream package creation, launch and uninstall under temporary roots.
Hosted Windows/Linux packaging and shim builds passed. The macOS path fixes
passed, but its self-extracting signing correction awaits a hosted rerun;
release publication is pending. Automatic branch builds run
only on `main`; release-tag pushes and manual builds follow the
[release contract](docs/RELEASES.md) for the tested baseline and verification commands.

The Windows shim source, bundled binaries, and rebuild instructions are under
[src/shim](src/shim/README.md). Rebuilding those assets requires LLVM.

MIT licensed; see [LICENSE.md](LICENSE.md). The initial extraction originated in
[kodybrown/dotask](https://github.com/kodybrown/dotask), commit
`27f268cee9e545a1a46292f873193732f628a219`.
