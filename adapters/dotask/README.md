# Use the prebuilt tools with dotask

This optional YAML `.task` adapter invokes `doinstall-builder` directly. It needs
only a dotask version supporting direct `exec` steps/options and the matching
DoInstall binaries; it compiles no Rust or C# task
and does not require the DoInstall source checkout. DoInstall's own Cargo build
and destination installer remain independent of dotask.

Copy `assemble.task` to your project's `.tasks/doinstall/assemble.task` (create
the parent directory first). Keep the two binaries together in a tool directory,
or specify separate explicit paths. From the consumer project root:

```powershell
dotask doinstall/assemble `
  --builder C:\Tools\doinstall\doinstall-builder.exe `
  --installer C:\Tools\doinstall\doinstall.exe `
  --config .tasks/resolved-installer.yaml `
  --output artifacts/my-app-installer
```

Use your actual tool/config/output paths. On Linux/macOS, omit `.exe` and use
host-matching binaries. Executable and relative argument paths resolve from the
consumer project's root, not the task file's directory. Arguments are separate
tokens, so paths containing spaces require no shell parsing inside the task.
The output must be a fresh directory. The task assembles but does not install.

For repeated use, configure literal target defaults in your existing
`.dotasks.yaml`, preserving its other entries:

```yaml
version: 1
targets:
  doinstall/assemble:
    defaults:
      builder: C:/Tools/doinstall/doinstall-builder.exe
      installer: C:/Tools/doinstall/doinstall.exe
      config: .tasks/resolved-installer.yaml
      output: artifacts/my-app-installer
```

Then run `dotask doinstall/assemble`; change the output for subsequent packages,
for example `dotask doinstall/assemble --output artifacts/my-app-1.1-installer`.
If your project already has `doinstall/assemble.rs`, choose a distinct task name
or invoke the `.task` extension explicitly to avoid implementation ambiguity.

The YAML must follow [DoInstall's resolved schema](../../docs/INSTALLATION.md):
application metadata, platform, architecture, payload, and runtime. Compilation,
producer tasks, gathers, source-only `build-steps`, and archive creation belong to
the caller. The source YAML used by dotask's existing Rust creator is a different
contract; do not pass it directly to this task.

Distribute the builder's complete output directory (`installer[.exe]`, YAML, and
payload). The builder and `.task` stay on the build machine. This direct process
task does not return dotask's typed `InstallerArtifact`; the existing Rust creator
and generic `_/installer/install` flow remain available for that integration.

To inspect this adapter without copying it into a consumer project:

```powershell
dotask --use-dir adapters/dotask help assemble
```

The older installed CLI that reports `.task` keys as only `description`,
`require_at_least_1_step`, and `steps` cannot run this adapter. Use a current native
CLI with direct-process `.task` support. During local verification the source
checkout's build launcher provided that CLI; the active installed dotask and PATH
were not modified.
