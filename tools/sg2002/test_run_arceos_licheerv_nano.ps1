$ErrorActionPreference = 'Stop'

$launcher = Join-Path $PSScriptRoot 'run_arceos_licheerv_nano.ps1'
$configurationJson = & $launcher -Example mushroom-web -CaptureSeconds 0 -ValidateOnly
if (-not $?) {
    throw 'Launcher validation failed.'
}
$configuration = $configurationJson | ConvertFrom-Json

if ($configuration.Example -cne 'mushroom-web') {
    throw "Unexpected example: $($configuration.Example)"
}
if ($configuration.WslDistribution -cne 'Ubuntu') {
    throw "Unexpected WSL distribution: $($configuration.WslDistribution)"
}
if ($configuration.ArceOsDirectory -cne '/home/chen/arceos-worktrees/sg2002-phone-tpu') {
    throw "Unexpected ArceOS directory: $($configuration.ArceOsDirectory)"
}
if ($configuration.Application -cne 'examples/mushroom-web-licheerv-nano') {
    throw "Unexpected application: $($configuration.Application)"
}
if ($configuration.ArtifactName -cne 'mushroom-web-licheerv-nano_riscv64-licheerv-nano.bin') {
    throw "Unexpected artifact: $($configuration.ArtifactName)"
}
if ($configuration.AppFeatures -cne 'hardware') {
    throw "Unexpected application features: $($configuration.AppFeatures)"
}

# The evidence capture has to stay byte-exact. ReadExisting() decoded with the
# port's default ASCII encoding and AppendAllText() re-encoded as UTF-8, so every
# byte >= 0x80 became '?' and a non-text UART stream could never be recorded
# faithfully from the log.
$launcherText = Get-Content -LiteralPath $launcher -Raw
foreach ($forbidden in @('ReadExisting', 'AppendAllText')) {
    if ($launcherText -match $forbidden) {
        throw "Launcher must not use $forbidden for evidence capture: it is not byte-exact."
    }
}
if ($launcherText -notmatch '\[System\.IO\.FileMode\]::Append') {
    throw 'Launcher must append evidence through an explicit byte FileStream.'
}
if ($launcherText -notmatch '\$capturePort\.Read\(') {
    throw 'Launcher must read raw evidence bytes from the capture port.'
}

$probeBytes = [byte[]](0x82, 0xB5, 0xF9, 0x00, 0xFF, 0x43)

# Document why the string path had to go: it cannot round-trip high bytes.
$legacyBack = [System.Text.Encoding]::ASCII.GetBytes(
    [System.Text.Encoding]::ASCII.GetString($probeBytes))
$legacyLossless = $true
for ($index = 0; $index -lt $probeBytes.Length; $index++) {
    if ($legacyBack[$index] -ne $probeBytes[$index]) {
        $legacyLossless = $false
    }
}
if ($legacyLossless) {
    throw 'The legacy string path was expected to be lossy but round-tripped cleanly.'
}

# Prove the byte path the launcher now uses really is lossless.
$probePath = Join-Path ([System.IO.Path]::GetTempPath()) (
    'sg2002-evidence-probe-{0}.bin' -f [guid]::NewGuid().ToString('N'))
$probeStream = [System.IO.File]::Open(
    $probePath,
    [System.IO.FileMode]::Append,
    [System.IO.FileAccess]::Write,
    [System.IO.FileShare]::ReadWrite)
try {
    $probeStream.Write($probeBytes, 0, $probeBytes.Length)
    $probeStream.Flush()
}
finally {
    $probeStream.Dispose()
}
try {
    $readBack = [System.IO.File]::ReadAllBytes($probePath)
    if ($readBack.Length -ne $probeBytes.Length) {
        throw "Evidence probe length mismatch: $($readBack.Length) vs $($probeBytes.Length)."
    }
    for ($index = 0; $index -lt $probeBytes.Length; $index++) {
        if ($readBack[$index] -ne $probeBytes[$index]) {
            throw "Evidence probe lost byte $index."
        }
    }
}
finally {
    Remove-Item -LiteralPath $probePath -Force -ErrorAction SilentlyContinue
}

Write-Output 'SG2002 launcher validation PASS'
