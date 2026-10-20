# Versioned tool releases

The release workflow builds standalone native doinstall tools with Cargo. It does
not need an installed doinstall, dotask, .NET SDK, or consumer project checkout.
Once the first release has been published, consumers can pin a GitHub release
instead of building this repository. Local acceptance passed on both release
platforms. Hosted Windows/Linux delivery acceptance and shim builds passed.
macOS CI builds, installer acceptance and release gating are temporarily disabled;
macOS packaging remains unverified. Release publication is pending.

## Release identity and compatibility

The root `Cargo.toml` package version is authoritative. Tags must be exactly
`vMAJOR.MINOR.PATCH`, for example `v0.1.0`, matching that version byte for byte.
Leading zeros, alternate prefixes, prerelease suffixes, four-component application
versions, and UTC application build stamps are not release tags. Application
packages can still use their own versions and build stamps as documented in
[INSTALLATION.md](INSTALLATION.md). Advance the Cargo version for each new release;
never move a published tag or replace existing release assets.

- Windows x64: `x86_64-pc-windows-msvc`, static CRT, tested by the workflow on
  `windows-2022` (Windows Server 2022). Local acceptance on Windows x64 is separate
  from the pending hosted runner run. No Windows ARM64 or older-Windows release
  compatibility is claimed.
- Linux x64: `x86_64-unknown-linux-gnu`, built and exercised on `ubuntu-22.04`,
  Ubuntu 22.04 / glibc 2.35. Native code uses the generic Rust x86_64 CPU baseline.
  A glibc dynamic loader, libc and libgcc_s are required; this is not a static or
  musl bundle. Release assembly refuses a different Linux build baseline and
  checks the tools' GLIBC symbol requirements do not exceed 2.35. Other Linux
  distributions, older glibc, Alpine/musl, macOS and ARM64 are not release-tested.

The pinned `rust-toolchain.toml` selects Rust **1.95.0**, rustfmt and clippy.
`Cargo.lock` fixes dependencies. GitHub Actions use full commit pins, with their
release versions annotated beside them. Runner images and OS security packages
continue to receive upstream updates; these builds are not claimed to be
bit-for-bit reproducible. Per-platform metadata records full `rustc --verbose`
output (including compiler commit/LLVM), Cargo version and target triple.

## Downloadable assets

For `v0.1.0` the release contains exactly:

```text
doinstall-v0.1.0-windows-x64-setup.exe
doinstall-v0.1.0-windows-x64-tools.zip
doinstall-v0.1.0-windows-x64-metadata.json
doinstall-v0.1.0-linux-x64-setup
doinstall-v0.1.0-linux-x64-tools.tar.gz
doinstall-v0.1.0-linux-x64-metadata.json
RELEASE-METADATA.json
SHA256SUMS
```

`setup[.exe]` is one self-extracting installer for people installing the tools.
The Linux download needs `chmod +x` after verification. The public delivery name
does not change the matching executable/YAML names embedded inside the launcher.
Run it interactively or use the normal temporary-root options described below.

The portable archive has **no enclosing directory**:

```text
doinstall[.exe]
doinstall-builder[.exe]
doinstall-launcher[.exe]
doinstall-server[.exe]
LICENSE.md
README.md
RELEASE.json
THIRD-PARTY.json
LICENSES/<crate>-<version>/<upstream license and notice files>
```

All four executables are from the same native build. Linux tar entries retain
executable permissions; no chmod is required after extracting that archive.
`RELEASE.json` records version, source commit, dirty-source flag, toolchain,
target, compatibility baseline and hashes of every other bundled file.
`THIRD-PARTY.json` maps dependencies to their supplied license texts. The
self-installer installs the identical portable payload, including these notices
and metadata. The launcher template stays beside the real builder; it is not an
installed command shim.

Per-platform metadata files duplicate `RELEASE.json` for inspection before
extraction. `RELEASE-METADATA.json` combines both platforms. `SHA256SUMS` covers
all seven other downloadable assets using SHA256, two spaces, filename, newline.
It does not include itself. Checksums detect corruption and mismatched assets;
they are not independent signatures. Pin the release tag, and for stronger
consumer locking also retain the reviewed archive hash in the consumer's own
configuration. Code signing and distribution signatures are not implemented.

## Download and verify a pinned release

These examples require the named release to have been published. Use a fresh
directory outside source/build trees. No persistent PATH changes or installation
are needed for portable tools.

Windows PowerShell:

```powershell
$ErrorActionPreference = 'Stop'
$version = '0.1.0'
$asset = "doinstall-v$version-windows-x64-tools.zip"
$base = "https://github.com/kodybrown/doinstall/releases/download/v$version"
$download = Join-Path $env:TEMP ('doinstall-download-' + [guid]::NewGuid())
New-Item -ItemType Directory $download | Out-Null
Invoke-WebRequest "$base/$asset" -OutFile (Join-Path $download $asset)
Invoke-WebRequest "$base/SHA256SUMS" -OutFile (Join-Path $download 'SHA256SUMS')
$entry = @(Get-Content (Join-Path $download 'SHA256SUMS') | Where-Object {
    $_ -match ('^[a-f0-9]{64}  ' + [regex]::Escape($asset) + '$')
})
if ($entry.Count -ne 1) { throw 'Missing or duplicate checksum' }
$expected = $entry[0].Substring(0, 64)
$actual = (Get-FileHash (Join-Path $download $asset) -Algorithm SHA256).Hash
if ($actual -ine $expected) { throw 'SHA256 mismatch' }
Expand-Archive (Join-Path $download $asset) (Join-Path $download 'tools')
$builder = Join-Path $download 'tools/doinstall-builder.exe'
$tools = & $builder --print-tool-paths | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) { throw 'Tool discovery failed' }
# $tools.Installer and $tools.LauncherTemplate are the real adjacent tools.
```

Linux bash (Ubuntu 22.04 x64):

```bash
set -euo pipefail
version=0.1.0
asset="doinstall-v${version}-linux-x64-tools.tar.gz"
base="https://github.com/kodybrown/doinstall/releases/download/v${version}"
download="$(mktemp -d)"
cd "$download"
curl --fail --location --proto '=https' "$base/$asset" --output "$asset"
curl --fail --location --proto '=https' "$base/SHA256SUMS" --output SHA256SUMS
sha256sum --check --ignore-missing SHA256SUMS
mkdir tools
tar -xzf "$asset" -C tools
./tools/doinstall-builder --print-tool-paths
```

Use that builder with explicit `--installer` and `--launcher-template` from the
query. For filekeep or another self-extracting producer, obtain host builder and
target runtime/template from the **same release**, follow the
[build-machine recipe](INSTALLATION.md#build-machine-recipe-for-people-and-ai-assistants),
and consume the builder's `--result-file` JSON `FilePath`. Do not guess delivery
names, compile doinstall per application, substitute command shims, or mix tools
from different versions. Cross-platform package assembly does not prove target
execution; exercise the resulting application installer on its target platform.

## Workflow and maintainer procedure

[release.yml](../.github/workflows/release.yml) automatically runs on commits pushed
to `main` and on `v*` release-tag pushes. Pushes to `develop` or other branches and
pull requests do not trigger it. Manual `workflow_dispatch` builds are accepted
only for `main`; other refs skip the build jobs. Before any build starts, version
tags must exactly match Cargo and point to a commit already reachable from remote
`main`. Native Windows and Linux jobs run the repository's full
`verify` command and release-script boundary tests, then create both deliveries.
Only Windows x64 and Linux x64 are built and exercised. The macOS verification
job is temporarily disabled, so it cannot generate test installers or block
artifact collection and publication. The LLVM shim rebuild check remains a gate.
macOS publishes no assets; re-enabling it requires native installer/signing
acceptance. The Rust shim hashes verify the checked-in source
and binaries; rebuilding with the runner's LLVM is a separate compilation check,
not a claim of byte-identical shim reproduction.

Each native job checks a closed file inventory, decompressed contents, private
path/credential patterns (UTF-8/ASCII and UTF-16), exact installed payload hashes,
and temporary-root install/package/launch/uninstall behavior. Source, Cargo-home,
home and Rust sysroot paths are remapped at compilation; Windows PDB paths are
neutralized and release symbols stripped. No checkout archives, PDBs, environment
dumps, credentials, receipts, user files, or developer installations are packaged.
Pattern scanning complements the allowlist; it cannot recognize every possible
secret. Dependency updates still need review. Missing license texts fail assembly;
the reviewed exceptions are documented under [packaging/licenses](../packaging/licenses/README.md).

Native jobs upload `native-windows` and `native-linux`. After all gates pass, a
read-only collection job verifies their checksums, version, platform and source
commit and uploads `release-ready`. Workflow artifacts expire after 14 days.
The archive inside the artifact preserves Linux modes, even though Actions'
outer artifact transport does not preserve executable modes.

Only a **push of a valid version tag on main's history** can enter the publishing
job. Manual runs on `main` only upload artifacts. All build/collection
jobs have `contents: read`, with checkout credentials disabled. Only publication
gets `contents: write`. It downloads the already-verified set, rechecks checksums,
uploads every asset into a new draft with `gh release create --verify-tag`, then
publishes the draft. No Rust compilation or repository code runs with the write
token. It refuses dirty-source artifacts. A rerun fails if that release already
exists; it never silently replaces assets. If upload/publication fails, inspect
the retained draft and workflow artifacts before deciding how to retry.

Before the first release, integrate the reviewed change from `develop` into
`main` through the repository's normal process, then push `main` for an
artifact-only rehearsal. The following are **commands for the maintainer after
authorization**, after local integration; implementing the workflow does not
execute them or switch the user's checkout:

```powershell
cd C:\Users\kodyb\Projects\doinstall
git status --short
git push origin main
gh run list --workflow release.yml --repo kodybrown/doinstall
# Copy the rehearsal run ID, then inspect its status and download its assets:
gh run watch RUN_ID --exit-status --repo kodybrown/doinstall
gh run download RUN_ID --name release-ready --dir C:/tmp/doinstall-first-release-review --repo kodybrown/doinstall
```

The repository's current default branch is `main`. GitHub requires a dispatchable
workflow to be present on the default branch before it can be manually dispatched.
Once this workflow reaches `main` through the normal authorized integration
process, manual artifact-only builds of `main` are also available:

```powershell
gh workflow run release.yml --ref main --repo kodybrown/doinstall
```

The normal push rehearsal above works without that prerequisite, runs the same
verification, and uploads `release-ready` without publishing. A valid tag push
also uses the workflow at the tagged commit. This implementation does not change
the default branch or repository settings.

After reviewing the successful rehearsal and obtaining release authorization:

```powershell
cd C:\Users\kodyb\Projects\doinstall
git status --short
python -B scripts/release.py version --tag v0.1.0
git tag -a v0.1.0 main -m 'doinstall v0.1.0'
git push origin refs/tags/v0.1.0
gh run list --workflow release.yml --repo kodybrown/doinstall
```

Tag the reviewed, clean commit on `main` that was rehearsed, with no intervening changes.
The tag push reruns every gate and publishes only if they all pass. GitHub Actions
and repository policy must permit the publishing job's contents write permission.
Do not create a separate release manually before the job, because existing
releases are deliberately refused.

## Local verification and acceptance

Developer prerequisites: the pinned Rust toolchain (rustup bootstraps it), a native
linker, Git, and Python 3.10+. Consumer bundles need none of these build tools.
All release output paths must be fresh and outside the checkout. Cargo's evaluated
`target_directory` and `build_directory` must also stay external; the existing
project output policy is preserved.

```powershell
cd C:\Users\kodyb\Projects\doinstall
.\build.cmd verify
python -B -m unittest discover -s scripts -p 'test_*.py'
python -B scripts/release.py build --output C:/tmp/doinstall-release-windows-review
git diff --check
```

On native Ubuntu 22.04 x64:

```bash
./build.sh verify
python3 -B -m unittest discover -s scripts -p 'test_*.py'
python3 -B scripts/release.py build --output /tmp/doinstall-release-linux-review
git diff --check
```

The release driver stages tools with the existing build helper and calls
`package --distribution-dir DIR --result-file FILE`. The distribution directory
supplies reviewed README, release metadata and dependency notices; its four tools
must byte-match the fresh build. `package` returns the builder's actual `FilePath`
in the report outside installer output. Ordinary `build package` behavior remains
available without distribution metadata.

Acceptance installs doinstall and an example application only under disposable
install/bin/source roots, explicitly sets `add-to-path=false`, enables no desktop
or menu shortcuts, and uninstalls both. Portable discovery and installed-builder
discovery are checked independently. Extracted sources are normally retained by
the launcher; the test owns and removes its entire temporary tree afterward.
Persistent user/machine PATH and active installations are never test targets.

Local release acceptance: Windows x64 and Ubuntu 22.04.5 x64 in Docker
(glibc 2.35, native Linux binaries). Docker shares its host kernel, so this does
not prove every Ubuntu kernel or bare-metal environment. The first hosted run
passed Linux packaging/acceptance and both shim builds. Its Windows/macOS failures
came from UI fixtures using unresolved temporary-path aliases; the fix resolves
fixture parents before creating temporary roots, including file-backed
integration and self-extracting unit fixtures. All 137 Windows tests also passed
with TEMP/TMP deliberately set through a directory junction. A subsequent hosted
run passed Windows/Linux deliveries, the shim builds, and
macOS's earlier path cases. macOS then reached self-extracting tests and failed
because assembly left an empty signature command and an unaccounted link-edit
tail. Further macOS failures remain unresolved, and macOS CI builds and installer
acceptance are now temporarily disabled. Those failures no longer gate the
Windows/Linux releases. macOS source and signing code remain available for future
work; successful native signing/execution is not claimed. Trusted distribution
signing and publication remain pending.
