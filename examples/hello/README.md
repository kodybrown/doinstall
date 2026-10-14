# Standalone hello example

This consumer compiles a tiny application with `rustc`, writes resolved YAML,
then calls the ordinary builder and installer. No dotask or .NET is involved.
Rust is needed on the build machine; the resulting delivery directory is standalone.
Use installer and builder binaries for the host platform and architecture.

From the repository root, build the tools and run the Windows example:

```powershell
.\build.cmd build
$target = (cargo metadata --format-version 1 --no-deps --locked | ConvertFrom-Json).target_directory
.\examples\hello\run.ps1 -Installer "$target/release/doinstall.exe" -Builder "$target/release/doinstall-builder.exe"
```

On Linux/macOS, run `./build.sh build`, locate `target_directory` with
`cargo metadata --format-version 1 --no-deps --locked`, then:

```sh
bash examples/hello/run.sh /absolute/target/release/doinstall /absolute/target/release/doinstall-builder
```

Both scripts assemble, validate, install, run the external command, and uninstall
using only fresh temporary application and command roots. They declare no shortcuts
and explicitly disable PATH writes. They clean their temporary example directory
on exit. Windows may print a separate relocated uninstaller directory; remove that
printed directory after the process exits if desired. Native Unix/ARM64 execution
of these scripts remains pending.

The caller owns application compilation, the frozen build stamp, YAML generation,
and any delivery archive. Replace `main.rs` with your project's build output and
adapt the generated YAML. Distribute the entire `delivery/` directory; do not
include `doinstall-builder`. Its recipient needs only the packaged installer and
any runtime required by the application. See [the YAML contract](../../docs/INSTALLATION.md).

## Interactive Windows preview

After building the tools, open a plain Cursive installer dialog:

```powershell
$target = (cargo metadata --format-version 1 --no-deps --locked | ConvertFrom-Json).target_directory
.\examples\hello\preview.ps1 -Installer "$target/release/doinstall.exe" -Builder "$target/release/doinstall-builder.exe"
```

Add `-SelfExtracting` to exercise the ZIP launcher and direct console handoff.
The precompiled `doinstall-launcher.exe` must be beside the builder. The same
temporary installation and locked-off PATH policy apply; extracted source
directories are intentionally retained outside the preview's cleanup root.

Tab/Shift+Tab moves between enabled controls; Space toggles a checkbox. Install
shows progress and a summary, and Close returns to the script. Esc/Ctrl+C asks before canceling
choices or requesting cancellation during work. Choose Yes to cancel; No or Escape
resumes the dialog. YAML `runtime.confirm-cancel: false` skips that confirmation. Keep edited locations beneath the
printed temporary preview root. The PATH option is locked off, and the script
removes its generated temporary directory after the dialog closes. It also runs
uninstall when the application remains at its initial preview location.

## Interactive Linux/macOS preview and WSL

Build and run Linux binaries inside WSL to test Linux behavior on Windows.
Enter Debian from PowerShell:

```powershell
wsl -d Debian
```

The Linux environment needs Rust 1.95 or newer, rustfmt, clippy, Git, and a C
linker. Windows Rust binaries do not replace the Linux toolchain.

In the WSL Bash shell, build from the ordinary Windows source directory and run
the debug preview. Cargo keeps the generated tools outside source:

```bash
cd /mnt/c/Users/kodyb/Projects/doinstall
cargo build --locked
cargo test --package doinstall --locked
env -u NO_COLOR bash examples/hello/preview.sh \
  /tmp/_rust/doinstall/target/debug/doinstall \
  /tmp/_rust/doinstall/target/debug/doinstall-builder
```

`preview.sh` opens the interactive form. To check the unattended lifecycle, use
`run.sh` with the same two tool paths.
The preview uses fresh temporary roots, locks PATH off, removes the initial
preview installation if present, and cleans up its generated directory on exit.
Keep edited paths under the printed root, as with the Windows preview.
If `NO_COLOR` is set, use `env -u NO_COLOR bash examples/hello/preview.sh ...`
for this preview without changing your shell settings. Resize the terminal to
check the notice and restoration, use Tab/Shift+Tab and Space, and check the Linux
Cancel/Install and No/Yes button order. WSL exercises Linux; macOS and other
terminal hosts still need their own acceptance.

Older Debian Git cannot read this Windows repository's `relativeWorktrees`
extension. That affects `build.sh verify` at its Git whitespace check, but does
not prevent these Cargo build/test commands or the preview. Full verification
needs a compatible Linux Git or a native Linux clone made from a portable bundle.

The same preview script works in native Linux/macOS Bash with tools built for
that host. Append `--self-extracting` to exercise the launcher, with the native
template beside the builder; extracted sources are intentionally retained.
The user reported this Linux x64 preview passed in Debian WSL using
a native `cargo build` debug binary. The combined UI/included-tools source also
passed all 79 Rust tests in Debian WSL. Linux release verification and
macOS/ARM64 execution for that historical preview were not performed.
The self-extracting follow-up passed 102 release tests and an interactive
launcher display/cancellation/terminal-restoration preview in Ubuntu x64 WSL;
Windows x64 passed 120 release tests and the equivalent launcher preview.
Native macOS/ARM64 execution remains pending.
