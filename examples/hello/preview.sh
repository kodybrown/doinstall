#!/usr/bin/env bash
set -euo pipefail
if [[ $# != 2 ]]; then
  echo 'Usage: bash examples/hello/preview.sh /absolute/path/doinstall /absolute/path/doinstall-builder' >&2
  exit 1
fi
installer=$1
builder=$2
[[ $installer = /* && $builder = /* ]] || { echo 'Tool paths must be absolute.' >&2; exit 1; }
source_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
case $(uname -s) in
  Linux) platform=linux ;;
  Darwin) platform=macos ;;
  *) echo 'Use preview.ps1 on Windows.' >&2; exit 1 ;;
esac
case $(uname -m) in
  x86_64) architecture=x64 ;;
  arm64|aarch64) architecture=arm64 ;;
  *) echo 'Unsupported host architecture.' >&2; exit 1 ;;
esac
stage=$(mktemp -d "${TMPDIR:-/tmp}/doinstall-preview-XXXXXXXX")
trap 'rm -rf -- "$stage"' EXIT
mkdir "$stage/payload"
rustc "$source_dir/main.rs" -o "$stage/payload/hello"
cat > "$stage/source.yaml" <<YAML
schema: 1
application:
  id: hello
  name: Hello Example
  version: 1.0.0
  description: An application packaged by DoInstall.
platform: $platform
architecture: $architecture
payload: payload
runtime:
  interactive: true
  commands:
    - name: hello
      executable: hello
  values:
    install-dir: $stage/application
    bin-dir: $stage/commands
    additional-command: false
    add-to-path: false
YAML
"$builder" --installer "$installer" --config "$stage/source.yaml" --output "$stage/delivery"
echo "Preview root: $stage"
echo 'Keep preview installation and command paths under this temporary root.'
# Lock PATH in the form; membership or edits never authorize a PATH write here.
status=0
"$stage/delivery/installer" --interactive --set add-to-path=false || status=$?
[[ $status = 0 || $status = 130 ]] || { echo 'Installer reported an error.' >&2; exit "$status"; }
if [[ -f $stage/application/installer/installation.yaml ]]; then
  "$stage/delivery/installer" uninstall --non-interactive --install-dir "$stage/application"
fi
