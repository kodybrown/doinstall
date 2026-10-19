# doinstall portable tools

Keep all four native tools together in this directory. No installation, Rust,
.NET, task runner, or PATH modification is required to use this bundle.

- `doinstall[.exe]`: target installer runtime.
- `doinstall-builder[.exe]`: build-machine package assembler.
- `doinstall-launcher[.exe]`: target self-extracting launcher template.
- `doinstall-server[.exe]`: optional web-stack helper.
- `LICENSE.md`, `LICENSES/`, `THIRD-PARTY.json`: project and dependency notices.
- `RELEASE.json`: version, source commit, compiler, target, and file SHA256 hashes.

Run `./doinstall-builder --print-tool-paths` (Windows:
`.\doinstall-builder.exe --print-tool-paths`) for JSON `Installer` and
`LauncherTemplate` paths. Use the real executable files when assembling packages.
Keep the matching builder, runtime, and template from the same release; for
cross-platform assembly use the builder for the host and tools for the target.
Consume the builder's `--result-file` JSON `FilePath` for the resulting delivery.

Release download, verification, compatibility, and packaging documentation:
https://github.com/kodybrown/doinstall/blob/develop/docs/RELEASES.md
https://github.com/kodybrown/doinstall/blob/develop/docs/INSTALLATION.md

Linux baseline: Ubuntu 22.04 x64, glibc 2.35. Windows: Server 2022 x64 CI.
Other distributions, older systems, musl, macOS, and ARM64 are not release-tested.
