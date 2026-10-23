param(
    [Parameter(Mandatory = $true)][string]$Installer,
    [Parameter(Mandatory = $true)][string]$Builder,
    [switch]$SelfExtracting,
    [string]$TerminalSize,
    [ValidateRange(6, 72)][Nullable[int]]$TerminalFontSize
)
$ErrorActionPreference = 'Stop'
$Installer = (Resolve-Path -LiteralPath $Installer).Path
$Builder = (Resolve-Path -LiteralPath $Builder).Path
$hostTriple = (& rustc -vV | Select-String '^host: ').ToString().Substring(6)
if ($LASTEXITCODE -ne 0) { throw 'rustc -vV failed' }
$architecture = if ($hostTriple.StartsWith('aarch64-')) { 'arm64' } elseif ($hostTriple.StartsWith('x86_64-')) { 'x64' } else { throw "Unsupported host: $hostTriple" }
$stage = Join-Path ([IO.Path]::GetTempPath()) ('doinstall-preview-' + [guid]::NewGuid())
$application = Join-Path $stage 'application'
New-Item -ItemType Directory -Path (Join-Path $stage 'payload') | Out-Null
try {
    & rustc (Join-Path $PSScriptRoot 'main.rs') -o (Join-Path $stage 'payload/hello.exe')
    if ($LASTEXITCODE -ne 0) { throw 'Example compilation failed' }
    # JSON is valid YAML and safely quotes paths containing spaces or backslashes.
    $config = @{
        schema = 1
        application = @{ id = 'hello'; name = 'Hello Example'; version = '1.0.0'; description = 'An application packaged by DoInstall.' }
        platform = 'windows'
        architecture = $architecture
        payload = 'payload'
        runtime = @{
            interactive = $true
            commands = @(@{ name = 'hello'; executable = 'hello.exe' })
            values = @{
                'install-dir' = $application
                'bin-dir' = (Join-Path $stage 'commands')
                'additional-command' = $false
                'add-to-path' = $false
            }
        }
    }
    $config | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $stage 'source.yaml') -Encoding utf8
    [string[]]$packageArguments = if ($SelfExtracting) { @('--self-extracting') } else { @() }
    & $Builder --installer $Installer --config (Join-Path $stage 'source.yaml') --output (Join-Path $stage 'delivery') @packageArguments
    if ($LASTEXITCODE -ne 0) { throw 'Assembly failed' }
    $packageName = if ($SelfExtracting) { 'hello-setup-v1.0.0-se.exe' } else { 'hello-setup-v1.0.0.exe' }
    $package = Join-Path (Join-Path $stage 'delivery') $packageName
    Write-Host "Preview root: $stage"
    Write-Host 'Keep preview installation and command paths under this temporary root.'
    # Make PATH immutable in the form so this preview never changes user PATH.
    # Presentation preferences are session-only. Unsupported hosts ignore them;
    # the preview's fixed PATH policy remains authoritative.
    [string[]]$presentationArguments = @()
    if ($TerminalSize) { $presentationArguments += @('--terminal-size', $TerminalSize) }
    if ($null -ne $TerminalFontSize) { $presentationArguments += @('--terminal-font-size', [string]$TerminalFontSize) }
    & $package --interactive --set add-to-path=false @presentationArguments
    if ($LASTEXITCODE -notin @(0, 130)) { throw 'Installer reported an error' }
    if (Test-Path -LiteralPath (Join-Path $application 'installer/installation.yaml')) {
        & $package uninstall --non-interactive --install-dir $application
        if ($LASTEXITCODE -ne 0) { throw 'Preview uninstall failed' }
    }
}
finally {
    $resolvedStage = [IO.Path]::GetFullPath($stage)
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (!$resolvedStage.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -or !(Split-Path $resolvedStage -Leaf).StartsWith('doinstall-preview-')) {
        throw "Refusing cleanup outside preview temporary root: $resolvedStage"
    }
    Remove-Item -LiteralPath $resolvedStage -Recurse
}
