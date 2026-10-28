# Versioned tool releases

The release workflow builds standalone native doinstall tools with Cargo. It does
not need an installed doinstall, dotask, .NET SDK, or consumer project checkout.
Consumers can pin a GitHub release instead of building this repository. Local
acceptance passed on both release platforms. Hosted Windows/Linux delivery
acceptance and shim builds passed.
macOS CI builds, installer acceptance and release gating are temporarily disabled;
macOS packaging remains unverified.

## Release identity and compatibility

The root `Cargo.toml` package version is authoritative. After main's verification
passes, Actions derives GitHub's internal release tag as `vMAJOR.MINOR.PATCH`,
for example `v0.1.0`, matching that version byte for byte. No manual tag is needed.
Leading zeros, alternate prefixes, prerelease suffixes, four-component application
versions, and UTC application build stamps are not release tags. Application
packages can still use their own versions and build stamps as documented in
[INSTALLATION.md](INSTALLATION.md). Advance the Cargo version for each new release;
never move a published tag or replace existing release assets.

- Windows x64: `x86_64-pc-windows-msvc`, static CRT, tested by the workflow on
  `windows-2022` (Windows Server 2022). Local acceptance on Windows x64 is separate
  from hosted runner acceptance. No Windows ARM64 or older-Windows release
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

## Workflow and publication

[release.yml](../.github/workflows/release.yml) automatically runs on pushes to
`main`. Develop, other branch pushes, pull requests and tag pushes do not trigger
it. A manual `workflow_dispatch` on main produces workflow artifacts without
publishing; other refs skip the jobs.

CI validates the committed MAJOR.MINOR.PATCH, verifies Windows x64 and Linux x64,
and assembles both portable tools and self-extracting installers. It never edits
the package version. Native jobs run the full Cargo-based verification and
release acceptance. The LLVM shim rebuild remains a required separate gate.
macOS is disabled; no macOS artifacts or compatibility are claimed.

Each native job checks the closed distribution inventory, decompressed contents,
private path/credential patterns (ASCII/UTF-8 and UTF-16), installed payload
hashes, and temporary-root install/package/launch/uninstall behavior. Source,
Cargo-home, home and Rust sysroot paths are remapped; Windows PDB paths are
neutralized. No checkout archives, PDBs, environment dumps, credentials, receipts,
user files or developer installations enter the distribution. Pattern scanning
complements the inventory; it cannot recognize every possible secret. Missing
license texts fail assembly; reviewed exceptions are in
[packaging/licenses](../packaging/licenses/README.md).

Native jobs upload `native-windows` and `native-linux`. After all gates pass,
read-only collection checks checksums, version, platform and source commit and
uploads `release-ready`. Workflow artifacts expire after 14 days. The nested
tar.gz preserves Linux executable modes despite the outer artifact transport.

Only a main push enters publication. Build/collection jobs have contents read
permission and disabled checkout credentials; only the publishing job receives
contents write. It downloads those same tested assets, verifies checksums, and
derives the release tag from their metadata. It creates the tag at the exact
tested commit while creating a draft release, uploads assets, then publishes.
There is no second tag-triggered build, Rust compilation, checkout or repository
code execution with the write token.

Existing version tags must resolve to the tested commit. A published release
for that commit is recognized through its tag and downloaded source metadata,
and a rerun leaves its assets untouched. Reusing a published version for different
source fails with instructions to increment the version. Network/authentication
errors fail rather than being interpreted as missing releases. Incomplete drafts
fail clearly and remain available for review; do not delete or overwrite them
automatically. Inspect the draft and workflow artifacts before choosing recovery.

Manual artifact-only review builds:

```powershell
gh workflow run release.yml --ref main --repo kodybrown/doinstall
gh run list --workflow release.yml --repo kodybrown/doinstall
gh run watch RUN_ID --exit-status --repo kodybrown/doinstall
gh run download RUN_ID --name release-ready --dir C:/tmp/doinstall-release-review --repo kodybrown/doinstall
```

## Local tasks and maintainer procedure

Optional managed task copies live in [.tasks/_/rust](../.tasks/_/rust/README.md).
Canonical sources are published in
[dotask's Rust catalog](https://github.com/kodybrown/dotask/tree/main/shared-tasks/rust).
Users call thin project tasks; shared sources remain product-independent.
Build/run/test call doinstall's Cargo-only helper so the same profile, path
remapping and inspection policy applies through either entry point. Managed
format/Clippy/version adapters remain unchanged.
Installed dotask is optional; Rust and Cargo are required for source builds. CI and release assembly bootstrap through Cargo without
installed dotask, Python, .NET or consumer checkouts.

Management created the minimal `.dotasks.yaml` project configuration and
`.dotasks-lock.yaml` ownership record; commit both with the installed copies.
The committed tasks run without catalog access.
To install into another initialized project, use `dotask --save rust` and
`dotask --add rust` without `--lang rust` (the public operations are YAML tasks
with Rust helpers). To update this project, run `dotask --sync --dry-run`, review,
then `dotask --sync`, test and commit the copies and tracking together. Do not
edit or format `.tasks/_/rust/` here; fix the originals in dotask, regenerate and
publish its catalog, then sync. Existing handwritten copies must be backed up
and moved aside before `--add`; it never adopts or overwrites untracked files.

1. Develop on develop, using `dotask build` and `dotask run`.
2. Occasionally run `dotask create-doinstall-installer` and explicitly launch its
   reported installer when you want to update your local installation.
3. Run `dotask test` and resolve failures.
4. Explicitly run `dotask update-version` for patch, or select minor/major.
5. Review and commit intended changes, including Cargo.toml, Cargo.lock and the
   changelog. The version task does not lock tested code; CI verifies the final
   committed version.
6. Fast-forward main through the normal process and push it (and develop as
   appropriate). Actions verifies both targets and publishes automatically.

```powershell
cd C:\Users\kodyb\Projects\doinstall
dotask build
dotask run --args '["--version"]'
dotask test
# Explicitly opt into the unchanged fully optimized release profile:
dotask build --profile release
dotask test --profile release
dotask update-version --dry-run
dotask update-version
# Alternatively choose ONE: --increment-minor or --increment-major.
# Review/commit, fast-forward main, and push through the normal Git process.
```

After reviewing and committing the intended changes on develop, start with a
clean checkout. These are maintainer commands, not actions performed by the
version task:

```powershell
git status --short
git switch main
git merge --ff-only develop
git push origin develop main
git switch develop
```

Stop if fast-forward promotion or the push fails; do not force or replace shared
history. Your work ends after the push. The Actions run and resulting release
remain visible on GitHub; publication requires every verification gate to pass.

Daily `build`, `run` and `test` default to Cargo's `dev` profile (incremental,
256 code-generation units, no optimization or debug information). Pass
`--profile release` for the unchanged fat-LTO, single-unit optimized profile.
`build.cmd build/run/verify` accept the same profile; `run --bin TOOL -- ARG...` or
`run --args JSON` forwards literal arguments. Tools go under evaluated Cargo
`debug/` or `release/` output; `build --stage-dir PATH` accepts an external directory.
Installer creation, `package`, `release build` and CI release verification always
use `release`; there is no local publishing command.

Launchers compile the build driver with the fast `bootstrap` profile, separate
from the `dev`/`release` executables that workspace tests rebuild. Direct Cargo
verification uses the same bootstrap profile; ordinary `build.cmd verify` and
`dotask test` select it automatically.

~~~powershell
cargo run --profile bootstrap --package doinstall-dev -- verify
~~~

Build formats source, runs strict Clippy, compiles with private-path remapping,
and scans all four tools. Run builds once and executes the exact scanned
doinstall binary. Its --args option is a JSON string array preserving literal
argument boundaries; the installed dotask cannot forward arbitrary trailing
arguments. Test builds once, runs workspace tests, checks shim integrity and
checks Git whitespace. All stop at the first failure.

Build/run/test/installer creation preserve the package version. Update-version
defaults to a patch increment; --increment is an explicit patch synonym.
--increment-minor resets patch, --increment-major resets minor/patch, and
--dry-run writes nothing. It edits only the package manifest and local lockfile
version, retaining unrelated bytes and dependencies. It does no build, commit,
merge, push, tag or publication. No local publish/tag-release tasks remain.

The package build script captures a UTC identity for human --version output:
MAJOR.MINOR.PATCH+yyMM.ddhh plus full UTC build time. Cargo, asset/install versions
and GitHub's derived tag use the SemVer core. The date has no SemVer precedence
and is never squeezed into Windows numeric version fields. Source, manifest or
lockfile changes refresh that identity; cached binaries retain it.
`RUST_BUILD_TIMESTAMP` permits an explicit timestamp. Daily wrappers never inject
a fresh clock value. Source/manifest changes refresh the shared tool identity;
unchanged builds keep the original executable bytes and timestamp. Generated
build records remain outside source and are independent of explicit version preparation.

Local packaging uses the freshly built builder/runtime/template/server and the
builder's actual result FilePath. It creates and inspects both native deliveries
and exercises temporary-root installations without changing active installs or
persistent PATH. The command itself does not install into your normal location:

```powershell
dotask create-doinstall-installer
# Cargo-only equivalent, with a fresh external output directory:
cargo run --quiet --locked --package doinstall-dev -- release build --output C:/tmp/doinstall-release-review
```

Default packaging output is a fresh directory beneath Cargo's external target
directory, printed on success. ZIP/tar.gz and installers are generated locally
or by Actions; never commit them. Windows packaging does not build/test Linux,
and Linux packaging does not build/test Windows. Each CI target compiles native
tools with its own UTC build time. Consumers download pinned release assets,
verify SHA256SUMS, and extract them without building doinstall.

## Local verification and acceptance

Developer prerequisites: the pinned Rust toolchain (rustup bootstraps it), a native
linker and Git. Consumer bundles need none of these build tools.
All release output paths must be fresh and outside the checkout. Cargo's evaluated
`target_directory` and `build_directory` must also stay external; the existing
project output policy is preserved.

```powershell
cd C:\Users\kodyb\Projects\doinstall
.\build.cmd verify --profile release
cargo run --quiet --locked --package doinstall-dev -- release build --output C:/tmp/doinstall-release-windows-review
git diff --check
```

On native Ubuntu 22.04 x64:

```bash
./build.sh verify --profile release
cargo run --quiet --locked --package doinstall-dev -- release build --output /tmp/doinstall-release-linux-review
git diff --check
```

Verification includes the Rust release helper's tests. For a focused helper run,
use `cargo test --package doinstall-dev --locked`. Generic shared-task tests now
belong to dotask: its checkout CLI installs from the catalog into a disposable
project and exercises build/run/test/version tasks without an installed dotask.
Publisher shell tests use a fake gh and never access real tags/releases.
Ordinary Cargo verification and native release acceptance remain independent.

The Rust release command stages one matching native build and passes the reviewed
README, metadata and dependency notices to the existing installer assembly
routine. It verifies that all four staged tools byte-match that payload. Assembly
invokes the real builder with a result file and consumes its actual `FilePath`;
it never guesses a delivery filename or uses the active installation. Ordinary
`build package` behavior remains available without distribution metadata.

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
signing remains pending.
