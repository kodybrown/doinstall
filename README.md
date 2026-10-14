# doinstall

A lightweight, cross-platform installer built in Rust.

Doinstall assembles application files and resolved YAML into an independent
installer directory. Its destination runtime installs immutable versioned builds,
creates owned commands/shortcuts, and retains an uninstaller. It works without
dotask, .NET, or a language SDK on the destination machine; applications may have
their own runtime requirements.

## Build from source

Rust 1.95+, Cargo, rustfmt, clippy, and a host linker are required.

```powershell
.\build.cmd build
.\build.cmd verify
```

Linux/macOS use `./build.sh build` and `./build.sh verify`. With no arguments the
launcher lists development commands. Outputs remain external; locate them with
`cargo metadata --format-version 1 --no-deps --locked` (`target_directory`).
Both `doinstall[.exe]` and `doinstall-builder[.exe]` are under `release/` there.
`build --stage-dir PATH` stages those tools and the optional `doinstall-server`
helper into a selected directory. The helper supports the reusable
[Debian/Ubuntu web stack recipe](docs/DEBIAN-WEB-STACK.md); the engine does not
require it.

## Create an installer

```sh
doinstall-builder --installer /tools/doinstall --config /project/installer.yaml --output /delivery/example-1.0
```

On Windows pass the corresponding `.exe` paths. The builder is a build-machine
tool; distribute only the resulting directory (`installer[.exe]`,
`installer.yaml`, and payload). The installer never runs producer tasks or reads
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

The Windows shim source, bundled binaries, and rebuild instructions are under
[src/shim](src/shim/README.md). Rebuilding those assets requires LLVM.

MIT licensed; see [LICENSE.md](LICENSE.md). The initial extraction originated in
[kodybrown/dotask](https://github.com/kodybrown/dotask), commit
`27f268cee9e545a1a46292f873193732f628a219`.
