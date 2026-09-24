$ErrorActionPreference = 'Stop'

$launcher = Join-Path $PSScriptRoot 'run_arceos_licheerv_nano.ps1'
$configurationJson = & $launcher -Example mushroom-web -ValidateOnly
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

Write-Output 'SG2002 launcher validation PASS'
