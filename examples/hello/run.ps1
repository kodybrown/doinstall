param(
    [Parameter(Mandatory = $true)][string]$Installer,
    [Parameter(Mandatory = $true)][string]$Builder
)
$ErrorActionPreference = 'Stop'
$Installer = (Resolve-Path -LiteralPath $Installer).Path
$Builder = (Resolve-Path -LiteralPath $Builder).Path
$hostTriple = (& rustc -vV | Select-String '^host: ').ToString().Substring(6)
if ($LASTEXITCODE -ne 0) { throw 'rustc -vV failed' }
$architecture = if ($hostTriple.StartsWith('aarch64-')) { 'arm64' } elseif ($hostTriple.StartsWith('x86_64-')) { 'x64' } else { throw "Unsupported host: $hostTriple" }
$stage = Join-Path ([IO.Path]::GetTempPath()) ('doinstall-example-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path (Join-Path $stage 'payload') | Out-Null
try {
    & rustc (Join-Path $PSScriptRoot 'main.rs') -o (Join-Path $stage 'payload/hello.exe')
    if ($LASTEXITCODE -ne 0) { throw 'Example compilation failed' }
    @"
schema: 1
application:
  id: hello
  name: Hello Example
  version: 1.0.0
  build:
    stamp: 26279-0612
platform: windows
architecture: $architecture
payload: payload
runtime:
  interactive: false
  commands:
    - name: hello
      executable: hello.exe
  values:
    add-to-path: false
"@ | Set-Content -LiteralPath (Join-Path $stage 'source.yaml') -Encoding utf8
    & $Builder --installer $Installer --config (Join-Path $stage 'source.yaml') --output (Join-Path $stage 'delivery')
    if ($LASTEXITCODE -ne 0) { throw 'Assembly failed' }
    $package = Join-Path $stage 'delivery/installer.exe'
    & $package --validate
    if ($LASTEXITCODE -ne 0) { throw 'Validation failed' }
    & $package --non-interactive --install-dir (Join-Path $stage 'application') --bin-dir (Join-Path $stage 'commands')
    if ($LASTEXITCODE -ne 0) { throw 'Installation failed' }
    & (Join-Path $stage 'commands/hello.exe')
    if ($LASTEXITCODE -ne 0) { throw 'Application failed' }
    & (Join-Path $stage 'application/installer/installer.exe') uninstall --non-interactive
    if ($LASTEXITCODE -ne 0) { throw 'Uninstall failed' }
    # Windows relocates a running retained uninstaller outside the app root.
    # Its printed temporary location can be removed after the process exits.
    Write-Host 'Example assembled, validated, installed, ran, and uninstalled.'
}
finally {
    # Verify the absolute generated root before recursively removing it.
    $resolvedStage = [IO.Path]::GetFullPath($stage)
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (!$resolvedStage.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -or !(Split-Path $resolvedStage -Leaf).StartsWith('doinstall-example-')) {
        throw "Refusing cleanup outside example temporary root: $resolvedStage"
    }
    Remove-Item -LiteralPath $resolvedStage -Recurse
}
