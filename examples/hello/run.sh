#!/usr/bin/env bash
set -euo pipefail
if [[ $# != 2 ]]; then
  echo "Usage: bash examples/hello/run.sh /absolute/path/doinstall /absolute/path/doinstall-builder" >&2
  exit 1
fi
installer=$1
builder=$2
[[ $installer = /* && $builder = /* ]] || { echo 'Tool paths must be absolute.' >&2; exit 1; }
source_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
case $(uname -s) in
  Linux) platform=linux ;;
  Darwin) platform=macos ;;
  *) echo 'Use run.ps1 on Windows.' >&2; exit 1 ;;
esac
case $(uname -m) in
  x86_64) architecture=x64 ;;
  arm64|aarch64) architecture=arm64 ;;
  *) echo 'Unsupported host architecture.' >&2; exit 1 ;;
esac
stage=$(mktemp -d "${TMPDIR:-/tmp}/doinstall-example-XXXXXXXX")
trap 'rm -rf -- "$stage"' EXIT
mkdir "$stage/payload"
rustc "$source_dir/main.rs" -o "$stage/payload/hello"
cat > "$stage/source.yaml" <<YAML
schema: 1
application:
  id: hello
  name: Hello Example
  version: 1.0.0
  build:
    stamp: 26279-0612
platform: $platform
architecture: $architecture
payload: payload
runtime:
  interactive: false
  commands:
    - name: hello
      executable: hello
  values:
    add-to-path: false
YAML
"$builder" --installer "$installer" --config "$stage/source.yaml" --output "$stage/delivery"
"$stage/delivery/installer" --validate
"$stage/delivery/installer" --non-interactive --install-dir "$stage/application" --bin-dir "$stage/commands"
"$stage/commands/hello"
"$stage/application/installer/installer" uninstall --non-interactive
echo 'Example assembled, validated, installed, ran, and uninstalled.'
