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
    [string] $ConfigPath = "$env:APPDATA\offload\config.json",
    [string] $RepoPath   = (Split-Path $PSScriptRoot -Parent),

    # Skip the cargo freshness check, which is the only slow one.
    [switch] $SkipBuild,

    # **The hotel-room mode.** Checks only what a cable can fix and prints the wiring map
    # when something is wrong. Skips the git and cargo checks entirely: those matter for a
    # measured run and are noise on a trip, where a NOT READY about a dirty working tree
    # teaches the operator to ignore the verdict.
    [switch] $Nightly
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


# Which physical path a device sits on. The mapping was established 2026-08-05 by moving one
# drive through every port type on this rig and watching the parent chain change:
#
#   PCIe hops   hub layers   ->  where
#       4           0            Element 5, PCIe tunnel
#       2           0            laptop LEFT port (Thunderbolt 4) - the only laptop port
#                                that can carry a PCIe tunnel at all
#       0           0            laptop RIGHT port (USB only; Dell recommends it for projectors)
#       0           1            Element 5 TB5 port
#       0           2            Element 5 USB-A port
#
# **An unreadable chain returns 'unknown' rather than guessing.** A check that spells "I could
# not tell" the same way it spells a real answer is the failure this project has already made
# twice - see REVIEWING.md, *A diagnostic that cannot fail*.
function Get-PathClass {
    param([int] $DiskNumber)

    $pnp = (Get-CimInstance Win32_DiskDrive -Filter "Index=$DiskNumber" -ErrorAction SilentlyContinue).PNPDeviceID
    if (-not $pnp) { return @{ Class = 'unknown'; Evidence = 'no PNPDeviceID for that disk index' } }

    $id = $pnp; $chain = @()
    while ($id -and $chain.Count -lt 9) {
        $dev = Get-PnpDevice -InstanceId $id -ErrorAction SilentlyContinue
        if (-not $dev) { break }
        $chain += $dev.FriendlyName
        $id = (Get-PnpDeviceProperty -InstanceId $id -KeyName 'DEVPKEY_Device_Parent' -ErrorAction SilentlyContinue).Data
    }
    if ($chain.Count -le 1) { return @{ Class = 'unknown'; Evidence = 'parent chain unreadable' } }

    $usb  = @($chain | Where-Object { $_ -match 'SuperSpeed USB Hub' }).Count
    $pcie = @($chain | Where-Object { $_ -match 'PCI Express (Up|Down)stream Switch Port' }).Count
    $ctrl = @($chain | Where-Object { $_ -match 'eXtensible Host Controller' })[0]
    $ctrlNum = if ($ctrl -match 'USB (\d+\.\d+)') { $Matches[1] } else { $null }

    $class = if     ($pcie -ge 4) { 'hub-pcie'   }
             elseif ($pcie -ge 2) { 'laptop-tb4' }
             elseif ($usb  -eq 1) { 'hub-tb5'    }
             elseif ($usb  -ge 2) { 'hub-usba'   }
             else                 { 'laptop-usb' }

    $ev = "$pcie PCIe hop(s), $usb hub layer(s)"
    if ($ctrlNum) { $ev += ", xHCI $ctrlNum" }
    @{ Class = $class; Evidence = $ev }
}

# Mirrors CONOPS.md's wiring table, and deliberately only the two rows with a consequence.
# Everything else is reported rather than asserted, because a check that fails on a harmless
# choice teaches you to ignore it.
$ExpectedPath = @{
    'OWC' = @{
        Class = @('laptop-tb4', 'hub-pcie')
        Why   = 'needs a PCIe tunnel; the laptop RIGHT port is USB-only and cannot carry one'
    }
    'SanDisk' = @{
        Class = @('hub-tb5')
        Why   = 'the only Gen 2x2 drive here - 1,486 MB/s on an Element 5 TB5 port against ~980 on any other port'
    }
}

# The wiring, printed rather than assumed. CONOPS.md measures two trips a year in bursts of
# eight nights, so night one is performed by someone who last did this six months ago.
# **Assume no memory of any of it.**
function Show-Wiring {
    ''
    '  THE RIG, as measured 2026-08-05. Two rows matter; the rest are forgiving.'
    ''
    '    MUST BE RIGHT'
    '      OWC enclosure   -> a laptop port on the LEFT SIDE'
    '                         the right-side port is USB-only and cannot carry its PCIe'
    '                         tunnel. If it comes up as "OWC Express 1M2" instead of'
    '                         "Seagate FireCuda", it is on the wrong port or needs a reseat.'
    '      SanDisk SSD     -> any TB5 port on the Element 5 (the three on the REAR)'
    '                         it is the only 20 Gbps drive: 1,486 MB/s there, ~980 anywhere'
    '                         else. Works fine on the wrong port, just slower.'
    ''
    '    ANYWHERE CONVENIENT'
    '      WD SSD          -> any port. It is 10 Gbps whatever you do, so it cannot be'
    '                         plugged in wrong.'
    '      SD card reader  -> any of the five 10 Gbps USB ports on the Element 5'
    '                         (two USB-C + two USB-A on the front, one USB-A on the rear).'
    '                         It needs 1.8 Gbps of a 10 Gbps port. All five are equal.'
    '      CFexpress rdr   -> a TB5 port on the Element 5.'
    '      Monitor         -> a TB5 port. Measured to cost the offload 0.7% - free.'
    ''
    '    Then: both cards in their readers, tracks in C:\Travel\GPX, and run  offload'
    ''
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

    # Trailing whitespace only. USB bridges pad the serial to a fixed-width SCSI field —
    # the SanDisk reports "2138FB400347    " where the config holds "2138FB400347" — so an
    # exact comparison misses every USB destination while Thunderbolt ones pass, which reads
    # exactly like two unplugged drives. Trimming further would be wrong: decision 6 keeps
    # serials verbatim because the OWC's really does end in a period, and stripping
    # punctuation is how two devices become one string.
    $disk = $disks | Where-Object { $_.SerialNumber -and $_.SerialNumber.Trim() -eq $dest.disk_serial.Trim() }
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

    # Where it is plugged in, which no other check can see. A drive on the wrong port works
    # perfectly and runs slow, which is this rig's recurring failure shape.
    $path = Get-PathClass -DiskNumber $disk.Number
    $want = $ExpectedPath[$dest.label]
    if ($want) {
        $ok = $path.Class -in $want.Class
        $detail = "$($path.Class)  [$($path.Evidence)]"
        if (-not $ok) { $detail += "  -- expected $($want.Class -join ' or '): $($want.Why)" }
        Report "  port $($dest.label)" $ok $detail
    }
    else {
        '      port {0,-16} {1}  [{2}]  (no requirement)' -f $dest.label, $path.Class, $path.Evidence
    }
}

# **Every expectation must name a destination that exists**, or the table above is
# decoration. `$ExpectedPath` is keyed by config label, so renaming a destination — which is
# exactly what replacing a drive involves — would silently stop checking the very device whose
# placement had just changed. Three lines to turn that from silent into loud.
foreach ($label in $ExpectedPath.Keys) {
    if ($label -notin @($config.destinations.label)) {
        Report "expectation $label" $false "no destination has this label — the port table is stale, so nothing checked it"
    }
}

# Four copies on fewer than four physical disks is the failure this assertion exists for.
$destDiskCount = ($config.destinations | ForEach-Object {
    $dest = $_
    if ($dest.path) {
        (Get-Partition -DriveLetter (Split-Path $dest.path -Qualifier).TrimEnd(':')).DiskNumber
    }
    else {
        # Bound outside the filter on purpose: inside a Where-Object block `$_` is the disk,
        # so `$_.disk_serial` would be silently null and every destination would match nothing.
        $wanted = $dest.disk_serial.Trim()
        ($disks | Where-Object { $_.SerialNumber -and $_.SerialNumber.Trim() -eq $wanted }).Number
    }
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

    # The PnP instance comes from the disk *index*, never from its name. Get-Disk and
    # Get-PnpDevice report different FriendlyNames for the same device — "SANDISK SDDR-409"
    # against "SANDISK SDDR-409 USB Device" — so matching on name silently returns nothing,
    # the parent chain comes back empty, and every card then falls into the "no USB in the
    # chain, must be PCIe" branch. That is how the USB 2.0 check below became a row that
    # could never fail, which is worse than not having it.
    $pnpId = (Get-CimInstance Win32_DiskDrive -Filter "Index=$($disk.Number)").PNPDeviceID
    $chain = if ($pnpId) { Get-ParentChain -InstanceId $pnpId } else { @() }

    # USB 2.0 is the silent 5.8x tax: the card mounts, every file reads, nothing errors.
    # A SuperSpeed device sits behind "Generic SuperSpeed USB Hub"; a USB 2.0 one behind
    # plain "Generic USB Hub". PCIe-tunnelled readers have no USB parent at all.
    # An empty chain means the lookup failed, not that the device is PCIe — say so rather
    # than reporting a link generation nothing was read from.
    $onUsb   = @($chain | Where-Object { $_ -match 'USB' }).Count -gt 0
    $slowHub = @($chain | Where-Object { $_ -eq 'Generic USB Hub' }).Count -gt 0

    $ok = ($chain.Count -gt 0) -and (-not $slowHub)
    $link = if ($chain.Count -eq 0) { 'link UNKNOWN — parent chain unreadable' }
            elseif ($slowHub)       { 'USB 2.0 — MOVE THE CABLE' }
            elseif ($onUsb)         { 'USB SuperSpeed' }
            else                    { "$($disk.BusType) — PCIe tunnelled" }

    Report "card $($card.DriveLetter):" $ok "$link · $($disk.FriendlyName)"
}

# ---- tracks ----------------------------------------------------------------

$tracks = @(Get-ChildItem $config.gpx_dir -Filter *.gpx -ErrorAction SilentlyContinue)
Report 'gpx tracks' ($tracks.Count -gt 0) "$($tracks.Count) in $($config.gpx_dir)"

# ---- the binary ------------------------------------------------------------

if ($Nightly) {
    ''
    '  (nightly mode: skipping the git and binary checks - they matter for a measured'
    '   run, not for tonight. Run without -Nightly before quoting any timing.)'
}
else {
Push-Location $RepoPath
try {
    $dirty = git status --porcelain
    Report 'working tree' ([string]::IsNullOrWhiteSpace($dirty)) `
        $(if ($dirty) { "$(($dirty -split "`n").Count) file(s) modified" } else { "clean at $(git rev-parse --short HEAD)" })

    if (-not $SkipBuild) {
        # A stale binary runs happily and misattributes the number to the wrong code.
        #
        # This *verifies*; it does not substitute for the clean build. docs/FULL-RUN.md has
        # `cargo clean` then `cargo build --release` before the reboot, so by the time this
        # runs the correct answer is "nothing to rebuild" — and anything else means source
        # changed after that build, which is exactly what needs catching.
        $build = cargo build --release 2>&1 | Out-String
        Report 'binary is HEAD''s' ($build -notmatch 'Compiling') `
            $(if ($build -match 'Compiling') { 'REBUILT — the binary was stale' } else { 'nothing to rebuild' })
    }
}
finally { Pop-Location }
}

# ---- the machine is idle ---------------------------------------------------

$running = Get-Process offload -ErrorAction SilentlyContinue
Report 'no run in flight' ($null -eq $running) `
    $(if ($running) { "offload pid $($running.Id) is already running" } else { 'none' })

# ---- verdict ---------------------------------------------------------------

''
if ($script:Failures -eq 0) {
    if ($Nightly) { 'READY — the rig is wired correctly. Run:  offload' }
    else          { 'READY — nothing read from a card or destination. Launch when you are.' }
    exit 0
}

"NOT READY — $($script:Failures) check(s) failed."
''
'  Every FAIL above is something a cable can fix, except the git and binary rows,'
'  which only matter when a timing is going to be quoted. Read the FAIL line: it names'
'  the device and what was expected of it.'

# The map, because a failure the operator cannot act on is the same as no check at all.
Show-Wiring

if (-not $Nightly) {
    '  (running with -Nightly skips the git and binary checks, which do not matter'
    '   on a trip.)'
    ''
}
exit 1
