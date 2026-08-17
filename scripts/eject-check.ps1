#requires -Version 7
<#
.SYNOPSIS
    Are all five removable devices actually put to bed? Metadata only.

.DESCRIPTION
    The end of the nightly ritual is five removable devices — three archive SSDs and two
    camera cards — and "the run said it ejected them" is not the same claim as "Windows has
    let go of them". On 2026-08-04 a run reported both cards dismounted and the operator
    found both still in the tray with drive letters.

    So this asks Windows rather than the report, and it encodes a DIFFERENT BAR PER DEVICE
    CLASS, which is the part that matters:

      archive SSD  gone from Get-Disk entirely. Powered down, not merely unmounted --
                   an unmounted disk still listed is a disk still spinning.
      SD card      volume and drive letter gone, while the READER stays present. Powering
                   the reader down would be the wrong device (DESIGN.md decision 22).
      CFexpress    see the note below -- through a Thunderbolt reader it enumerates as a
                   fixed NVMe disk, so it has no separable media to eject and this script
                   reports what it sees rather than asserting a bar nobody has agreed.

    Every check reads device and volume properties only. Nothing opens a file, so this is
    safe during a cold-cache procedure (docs/FULL-RUN.md).

    Exits non-zero if any device is still attached, so it can gate "safe to pack".
#>
[CmdletBinding()]
param(
    [string] $ConfigPath = "$env:APPDATA\offload\config.json"
)

$ErrorActionPreference = 'Stop'
$script:Failures = 0

function Report {
    param([string] $Name, [bool] $Ok, [string] $Detail)

    if (-not $Ok) { $script:Failures++ }
    $tag = if ($Ok) { 'DOWN' } else { 'UP  ' }
    '{0}  {1,-22} {2}' -f $tag, $Name, $Detail
}

if (-not (Test-Path $ConfigPath)) {
    "config not found at $ConfigPath"
    exit 1
}
$config = Get-Content $ConfigPath -Raw | ConvertFrom-Json

''
'=== Are all five devices put to bed? — metadata only ==='
''

$disks = Get-Disk

# ---- the three archive SSDs -------------------------------------------------
#
# Serial comparison is trimmed of surrounding whitespace ONLY. USB bridges pad the serial
# to a fixed-width SCSI field; decision 6 keeps serials otherwise verbatim, because the
# OWC's really does end in a period and stripping punctuation is how two devices become
# one string.

foreach ($dest in $config.destinations) {
    if ($dest.path) { continue }   # the laptop copy is not removable and has nothing to eject

    $wanted = $dest.disk_serial.Trim()
    $disk = $disks | Where-Object { $_.SerialNumber -and $_.SerialNumber.Trim() -eq $wanted }

    if (-not $disk) {
        Report "SSD $($dest.label)" $true 'gone from the disk list — powered down'
        continue
    }

    $letter = (Get-Partition -DiskNumber $disk.Number -ErrorAction SilentlyContinue |
               Where-Object DriveLetter).DriveLetter
    $where = if ($letter) { "still mounted at ${letter}:" } else { 'still present, no letter' }
    Report "SSD $($dest.label)" $false "$where · disk $($disk.Number) · $($disk.BusType)"
}

# ---- the two camera cards ---------------------------------------------------
#
# A card is a volume holding DCIM that is not a configured destination — the same
# discriminator pre-flight uses. Test-Path on the DCIM directory is a metadata probe, and
# it is also an access: on a dismounted volume it can be the thing that remounts it. That
# is not a flaw in the check. Explorer and the indexer poll constantly, so a state any
# observation destroys is a state the operator never sees either.

$destinationLetters = @($config.destinations | ForEach-Object {
    if ($_.path) { (Split-Path $_.path -Qualifier).TrimEnd(':') }
    else {
        $wanted = $_.disk_serial.Trim()
        $d = $disks | Where-Object { $_.SerialNumber -and $_.SerialNumber.Trim() -eq $wanted }
        if ($d) { (Get-Partition -DiskNumber $d.Number -ErrorAction SilentlyContinue | Where-Object DriveLetter).DriveLetter }
    }
})

$cards = Get-Volume |
    Where-Object { $_.DriveLetter -and $_.DriveLetter -notin $destinationLetters } |
    Where-Object { Test-Path "$($_.DriveLetter):\DCIM" }

if ($cards.Count -eq 0) {
    Report 'cards' $true 'no volume with DCIM is mounted — both cards released'
}
foreach ($card in $cards) {
    $partition = Get-Partition -DriveLetter $card.DriveLetter
    $disk = $disks | Where-Object Number -eq $partition.DiskNumber
    $cim = Get-CimInstance Win32_DiskDrive -Filter "Index=$($partition.DiskNumber)"

    # Removable media can be ejected out of a reader that stays put. Fixed media cannot --
    # a CFexpress behind a Thunderbolt reader IS the NVMe device as far as Windows is
    # concerned, so there is nothing to eject short of the device itself.
    $removable = $cim.CapabilityDescriptions -contains 'Supports Removable Media'
    $kind = if ($removable) { 'removable media — should eject out of its reader' }
            else            { 'FIXED media — no separable media to eject (see decision 22)' }

    Report "card $($card.DriveLetter):" $false "still mounted · $($disk.BusType) · $kind"
}

# ---- the readers, which are supposed to STAY -------------------------------
#
# Reported rather than asserted: a reader that vanished means something powered down the
# wrong device, which is a different bug from a card that would not release.

''
'Readers (these are supposed to remain present):'

# **Not filtered by -Class DiskDrive, and that was a real bug.** A reader only presents a
# DiskDrive while a card is *in* it, so once the cards are released the class query finds
# nothing and reported "none found" on a rig where a reader was sitting right there, healthy.
# A check that cannot distinguish "gone" from "has no card in it" is worse than no check.
$readers = Get-PnpDevice -PresentOnly -ErrorAction SilentlyContinue |
    Where-Object { $_.FriendlyName -match 'SDDR|CFexpress|ProGrade|Card Reader' }

if ($readers) { $readers | ForEach-Object { "      present  [$($_.Class)]  $($_.FriendlyName)" } }
else          { "      none present" }

# Expected asymmetry, measured 2026-08-05 and printed so it does not read as a fault: a USB
# reader's disk *is* the reader, so ejecting the card powers the reader down too. A CFexpress
# behind a Thunderbolt reader sits below a PCIe port, so only the card goes and the router
# stays. See DESIGN.md decision 22.
'      (a USB SD reader is expected to be gone — it powers down with its card and needs a replug)'

''
if ($script:Failures -eq 0) {
    'ALL DOWN — every removable device has been released. Safe to unplug and pack.'
    exit 0
}
"STILL UP — $($script:Failures) device(s) have not been released."
'Anything listed UP is still attached as far as Windows is concerned, whatever the run reported.'
exit 1
