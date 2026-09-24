[CmdletBinding()]
param(
    [ValidateSet('mushroom-web')]
    [string]$Example = 'mushroom-web',
    [string]$WslDistribution = 'Ubuntu',
    [string]$ArceOsDirectory = '/home/chen/arceos-worktrees/sg2002-phone-tpu',
    [string]$SenderPath = '',
    [string]$FirmwareDirectory = '',
    [string]$LogPath = '',
    [ValidateRange(1, 3600)]
    [int]$UbootWaitSeconds = 600,
    [switch]$ValidateOnly
)

$ErrorActionPreference = 'Stop'
$env:PYTHONUTF8 = '1'

$platform = 'axplat-riscv64-licheerv-nano'
$application = 'examples/mushroom-web-licheerv-nano'
$artifactName = 'mushroom-web-licheerv-nano_riscv64-licheerv-nano.bin'
$appFeatures = 'hardware'
$binaryInWsl = "$ArceOsDirectory/$application/$artifactName"
$binaryInWindows = "\\wsl.localhost\$WslDistribution$ArceOsDirectory\$($application.Replace('/', '\'))\$artifactName"

$configuration = [ordered]@{
    Example = $Example
    WslDistribution = $WslDistribution
    ArceOsDirectory = $ArceOsDirectory
    Application = $application
    ArtifactName = $artifactName
    AppFeatures = $appFeatures
    BinaryInWsl = $binaryInWsl
    BinaryInWindows = $binaryInWindows
}
if ($ValidateOnly) {
    $configuration | ConvertTo-Json -Compress
    return
}

if ([string]::IsNullOrWhiteSpace($SenderPath)) {
    $SenderPath = Join-Path $PSScriptRoot 'send_arceos_xmodem.py'
}
if (-not (Test-Path -LiteralPath $SenderPath -PathType Leaf)) {
    throw "XMODEM sender was not found. Pass its exact path with -SenderPath: $SenderPath"
}
if ([string]::IsNullOrWhiteSpace($FirmwareDirectory)) {
    $FirmwareDirectory = Join-Path (Split-Path -Parent $SenderPath) '.local-firmware'
}

$firmwareManifest = @(
    [pscustomobject]@{ Name = 'fw_adid_8800d80_u02.bin'; Length = 1708; Sha256 = 'a526cbd02fcdc495f049f3ad6b5933cb08cd984b16790c716a060d582fee1a56' }
    [pscustomobject]@{ Name = 'fw_patch_8800d80_u02.bin'; Length = 32700; Sha256 = 'f0e2f5bbc17bc327ca7f1574ff55370dfd863d931514347bb4abc18a74f6218f' }
    [pscustomobject]@{ Name = 'fw_patch_table_8800d80_u02.bin'; Length = 1384; Sha256 = '9decb77435b7e9713e33e32da483d683b7329ed93b672b2d1b134031d7da5f67' }
    [pscustomobject]@{ Name = 'fmacfwbt_8800d80_h_u02.bin'; Length = 329580; Sha256 = 'c84225728b962510fbeb420f062cb2d07f7179d49f344b8b4741ac9937d84f01' }
    [pscustomobject]@{ Name = 'fw_patch_8800d80_u02_ext0.bin'; Length = 16136; Sha256 = 'ca738f5ea5aca6021c4f48b60ff35e835dfdaf072a8490e3ba8ba0a42440e3e5' }
)
foreach ($entry in $firmwareManifest) {
    $path = Join-Path $FirmwareDirectory $entry.Name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Official AIC8800 firmware was not found: $path"
    }
    $length = (Get-Item -LiteralPath $path).Length
    if ($length -ne $entry.Length) {
        throw "AIC8800 firmware size mismatch for $($entry.Name); expected $($entry.Length), actual $length."
    }
    $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($hash -cne $entry.Sha256) {
        throw "AIC8800 firmware SHA-256 mismatch for $($entry.Name); expected $($entry.Sha256), actual $hash."
    }
}

$serialDevice = Get-PnpDevice -Class Ports -PresentOnly |
    Where-Object { $_.InstanceId -like 'USB\VID_1A86&PID_7523\*' } |
    Select-Object -First 1
if ($null -eq $serialDevice -or $serialDevice.Status -cne 'OK') {
    throw 'The CH340 USB serial adapter is not connected or is not ready.'
}
if ($serialDevice.FriendlyName -notmatch '\((COM[0-9]+)\)$') {
    throw "The CH340 port name could not be extracted from: $($serialDevice.FriendlyName)"
}
$serialPortName = $Matches[1]

$pythonLauncher = (Get-Command py.exe -ErrorAction Stop).Source
& $pythonLauncher -3.12 -c 'import paramiko, serial, xmodem'
if ($LASTEXITCODE -ne 0) {
    throw 'Python 3.12 with paramiko, pyserial, and xmodem is required.'
}

Write-Host "[1/3] Building '$Example' for LicheeRV Nano..."
$buildCommand = "cd '$ArceOsDirectory' && make A=$application MYPLAT=$platform APP_FEATURES=$appFeatures build"
& wsl.exe -d $WslDistribution -- bash -lic $buildCommand
if ($LASTEXITCODE -ne 0) {
    throw "ArceOS build failed with exit code $LASTEXITCODE."
}
& wsl.exe -d $WslDistribution -- test -s $binaryInWsl
if ($LASTEXITCODE -ne 0) {
    throw "Build artifact was not created: $binaryInWsl"
}

if ([string]::IsNullOrWhiteSpace($LogPath)) {
    $timestamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $LogPath = Join-Path $PSScriptRoot "logs\mushroom-web-sta-board-$timestamp.log"
}
$logDirectory = Split-Path -Parent $LogPath
if (-not [string]::IsNullOrWhiteSpace($logDirectory)) {
    New-Item -ItemType Directory -Force -Path $logDirectory | Out-Null
}

Write-Host "[2/3] Serial port: $serialPortName"
Write-Host "UART log: $LogPath"
Write-Host '[3/3] Press RESET once. The Wi-Fi password prompt is hidden and is never put on the command line.'

$senderArguments = @(
    '-3.12'
    $SenderPath
    "--port=$serialPortName"
    "--uboot-wait-seconds=$UbootWaitSeconds"
    '--wait-uboot'
    '--go'
    "--aic-firmware-directory=$FirmwareDirectory"
    '--wifi-credentials-prompt'
    $binaryInWindows
)
& $pythonLauncher @senderArguments 2>&1 | Tee-Object -FilePath $LogPath
$senderExitCode = $LASTEXITCODE
if ($senderExitCode -ne 0) {
    throw "Board launch failed with exit code $senderExitCode. See $LogPath"
}

$urlLine = Select-String -LiteralPath $LogPath -Pattern '^MUSHROOM_WEB_URL http://[^/]+/$' |
    Select-Object -Last 1
if ($null -eq $urlLine) {
    throw "The board did not print MUSHROOM_WEB_URL. See $LogPath"
}
Write-Host "Board is ready: $($urlLine.Line.Substring('MUSHROOM_WEB_URL '.Length))"
