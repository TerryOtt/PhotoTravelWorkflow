#requires -Version 7
<#
.SYNOPSIS
    Metadata-only rig checks before a measured end-to-end run. See docs/FULL-RUN.md.

.DESCRIPTION
    Every check here reads device and volume *properties* — PnP enumeration, disk
    attributes, whether a directory exists. None of it opens a file on a card or a
    destination, so running this does not warm the caches a reboot just cleared.

    That property is the reason this script exists at all: the checks were previously
    improvised at the keyboard, and improvising is how a session ends up walking archive
    trees it was supposed to leave alone. A fixed, reviewed set of checks removes the
    temptation by leaving nothing to decide.

    Exits non-zero if any check fails, so it can gate the run.
#>
[CmdletBinding()]
param(
    [string] $ConfigPath = "$env:APPDATA\photoday\config.json",
    [string] $RepoPath   = (Split-Path $PSScriptRoot -Parent),

    # Skip the cargo freshness check, which is the only slow one.
    [switch] $SkipBuild
)

$ErrorActionPreference = 'Stop'
$script:Failures = 0

function Report {
    param([string] $Name, [bool] $Ok, [string] $Detail)

    if (-not $Ok) { $script:Failures++ }
    $tag = if ($Ok) { 'PASS' } else { 'FAIL' }
    '{0}  {1,-30} {2}' -f $tag, $Name, $Detail
}

# The PnP parent chain is the evidence for link generation. The storage protocol name
# (BOT vs UAS) is not — both card readers here report plain "USB Mass Storage Device"
# while one of them sustains 222 MB/s, which USB 2.0 cannot physically carry.
function Get-ParentChain {
    param([string] $InstanceId)

    $chain = @()
    $id = $InstanceId
    while ($id -and $chain.Count -lt 8) {
        $dev = Get-PnpDevice -InstanceId $id -ErrorAction SilentlyContinue
        if (-not $dev) { break }
        $chain += $dev.FriendlyName
        $id = (Get-PnpDeviceProperty -InstanceId $id -KeyName 'DEVPKEY_Device_Parent' `
               -ErrorAction SilentlyContinue).Data
    }
    $chain
}

function Get-DriveLetterForDisk {
    param([int] $DiskNumber)

    (Get-Partition -DiskNumber $DiskNumber -ErrorAction SilentlyContinue |
        Where-Object DriveLetter).DriveLetter
}

''
'=== Rig checks — metadata only, no file data read ==='
''

# ---- config ----------------------------------------------------------------

if (-not (Test-Path $ConfigPath)) {
    Report 'config' $false "not found at $ConfigPath"
    exit 1
}
$config = Get-Content $ConfigPath -Raw | ConvertFrom-Json
Report 'config' $true $ConfigPath

$disks = Get-Disk
$destinationLetters = @()

# ---- destinations ----------------------------------------------------------

foreach ($dest in $config.destinations) {
    if ($dest.path) {
        $letter = (Split-Path $dest.path -Qualifier).TrimEnd(':')
        $destinationLetters += $letter
        Report "dest $($dest.label)" (Test-Path $dest.path) "$($dest.path) — on this machine's disk"
        continue
    }

    $disk = $disks | Where-Object SerialNumber -eq $dest.disk_serial
    if (-not $disk) {
        # A Thunderbolt enclosure that falls back to its USB bridge reports the *bridge's*
        # serial instead of the NVMe's, so a missing serial is very often a downgraded
        # link rather than an absent disk. Say so, because the two want opposite actions.
        $detail = "serial $($dest.disk_serial) not present — disk absent, or its enclosure fell back to USB (reseat and re-check)"
        Report "dest $($dest.label)" $false $detail
        continue
    }

    $letter = Get-DriveLetterForDisk -DiskNumber $disk.Number
    $destinationLetters += $letter
    Report "dest $($dest.label)" $true "$($letter): disk $($disk.Number) · $($disk.BusType) · $($disk.FriendlyName)"
}

# Four copies on fewer than four physical disks is the failure this assertion exists for.
$destDiskCount = ($config.destinations | ForEach-Object {
    if ($_.path) { (Get-Partition -DriveLetter (Split-Path $_.path -Qualifier).TrimEnd(':')).DiskNumber }
    else { ($disks | Where-Object SerialNumber -eq $_.disk_serial).Number }
} | Where-Object { $null -ne $_ } | Sort-Object -Unique).Count

Report 'destinations distinct' ($destDiskCount -eq $config.destinations.Count) `
    "$destDiskCount distinct disks for $($config.destinations.Count) destinations"

# ---- cards -----------------------------------------------------------------

# A card is a volume holding DCIM that is not a configured destination — the same
# discriminator pre-flight uses. Removability is not usable evidence: the two readers
# here disagree about it for identical cards.
$cards = Get-Volume |
    Where-Object { $_.DriveLetter -and $_.DriveLetter -notin $destinationLetters } |
    Where-Object { Test-Path "$($_.DriveLetter):\DCIM" }

Report 'cards found' ($cards.Count -eq 2) "$($cards.Count) volume(s) with DCIM"

foreach ($card in $cards) {
    $partition = Get-Partition -DriveLetter $card.DriveLetter
    $disk = $disks | Where-Object Number -eq $partition.DiskNumber
    $pnp = Get-PnpDevice -PresentOnly -Class DiskDrive |
        Where-Object { $_.FriendlyName -eq $disk.FriendlyName }
    $chain = if ($pnp) { Get-ParentChain -InstanceId $pnp[0].InstanceId } else { @() }

    # USB 2.0 is the silent 5.8x tax: the card mounts, every file reads, nothing errors.
    # A SuperSpeed device sits behind "Generic SuperSpeed USB Hub"; a USB 2.0 one behind
    # plain "Generic USB Hub". PCIe-tunnelled readers have no USB parent at all.
    $onUsb = $chain -match 'USB'
    $slowHub = ($chain | Where-Object { $_ -eq 'Generic USB Hub' }).Count -gt 0
    $ok = (-not $onUsb) -or (-not $slowHub)
    $link = if (-not $onUsb) { "$($disk.BusType) — PCIe tunnelled" }
            elseif ($slowHub) { 'USB 2.0 — MOVE THE CABLE' }
            else { 'USB SuperSpeed' }

    Report "card $($card.DriveLetter):" $ok "$link · $($disk.FriendlyName)"
}

# ---- tracks ----------------------------------------------------------------

$tracks = @(Get-ChildItem $config.gpx_dir -Filter *.gpx -ErrorAction SilentlyContinue)
Report 'gpx tracks' ($tracks.Count -gt 0) "$($tracks.Count) in $($config.gpx_dir)"

# ---- the binary ------------------------------------------------------------

Push-Location $RepoPath
try {
    $dirty = git status --porcelain
    Report 'working tree' ([string]::IsNullOrWhiteSpace($dirty)) `
        $(if ($dirty) { "$(($dirty -split "`n").Count) file(s) modified" } else { "clean at $(git rev-parse --short HEAD)" })

    if (-not $SkipBuild) {
        # A stale binary runs happily and misattributes the number to the wrong code.
        $build = cargo build --release 2>&1 | Out-String
        Report 'binary is HEAD''s' ($build -notmatch 'Compiling') `
            $(if ($build -match 'Compiling') { 'REBUILT — the binary was stale' } else { 'nothing to rebuild' })
    }
}
finally { Pop-Location }

# ---- the machine is idle ---------------------------------------------------

$running = Get-Process photoday -ErrorAction SilentlyContinue
Report 'no run in flight' ($null -eq $running) `
    $(if ($running) { "photoday pid $($running.Id) is already running" } else { 'none' })

# ---- verdict ---------------------------------------------------------------

''
if ($script:Failures -eq 0) {
    'READY — nothing read from a card or destination. Launch when you are.'
    exit 0
}
"NOT READY — $($script:Failures) check(s) failed. Fix before launching; a run started now"
'is not comparable with the numbers in DESIGN.md.'
exit 1
