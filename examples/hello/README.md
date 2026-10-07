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
