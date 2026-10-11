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
`build --stage-dir PATH` copies the release pair into a selected tool directory.

## Create an installer

```sh
doinstall-builder --installer /tools/doinstall --config /project/installer.yaml --output /delivery/example-1.0
```

On Windows pass the corresponding `.exe` paths. The builder is a build-machine
tool; distribute only the resulting directory (`installer[.exe]`,
`installer.yaml`, and payload). The installer never runs producer tasks or reads
`.dotasks.yaml`. See [the configuration and ownership contract](docs/INSTALLATION.md).

Try the [runnable standalone hello example](examples/hello/README.md), which
assembles, validates, installs, runs, and uninstalls entirely under temporary roots.

Applications can select any build system. Dotask's optional `_/doinstall/*`
adapters invoke these ordinary tools and its generic `_/installer/install`
launcher runs the returned artifact. Doinstall does not depend on those adapters.

## Interactive installer

Install and uninstall use a plain Cursive dialog with standard controls and
keyboard navigation when interactive execution is selected. Choices, progress,
and summary occupy separate screens. Cancel/Escape/Ctrl+C confirms by default;
`runtime.confirm-cancel: false` skips the confirmation. `--plain` uses the original line prompts;
`--non-interactive` keeps automation output plain. No theme configuration is added.
Try the [temporary Windows UI preview](examples/hello/README.md#interactive-windows-preview).

## Development and acceptance

`verify` runs formatting, Rust tests, clippy, shim hashes, and Git whitespace.
Tests use disposable install/command/shortcut directories and a disposable
registry key; they do not modify an active installation or the user's PATH.
The inherited Windows x64 runtime coverage is retained. Native Linux/macOS and
Windows ARM64 acceptance remain pending. Source-supported platforms are not
claims of native acceptance.

The Windows shim source, bundled binaries, and rebuild instructions are under
[src/shim](src/shim/README.md). Rebuilding those assets requires LLVM.

MIT licensed; see [LICENSE.md](LICENSE.md). The initial extraction originated in
[kodybrown/dotask](https://github.com/kodybrown/dotask), commit
`27f268cee9e545a1a46292f873193732f628a219`.
