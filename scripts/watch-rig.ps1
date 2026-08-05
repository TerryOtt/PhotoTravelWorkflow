#requires -Version 7
<#
.SYNOPSIS
    Emit one line per change in rig state. Metadata only. Intended for Claude's Monitor tool.

.DESCRIPTION
    Standing project guidance (2026-08-05, from the operator): **watch the rig rather than
    waiting to be told.** Plugging a cable in should be the event that starts the work, not a
    thing he has to narrate — "plug the OWC in", then several minutes of him confirming and
    me asking.

    Every line printed is a change. Silence means nothing changed, which is why the poll
    prints nothing on a steady rig and cannot flood the conversation.

    WHAT IT WATCHES, and each row is here because it cost this project time:

      + / -  a disk arriving or leaving      an eject, a replug, a reader powering down
      !      BusType changing on one disk    the OWC bridging to USB instead of PCIe, which
                                             is silent, costs 3x, and looks exactly like
                                             hardware gone slow (DESIGN.md decision 22 notes)
      + / -  a drive letter                  letters move across reboots (decision 6)

    WHY POLLING AND NOT A WMI EVENT SUBSCRIPTION. Win32_VolumeChangeEvent is the push
    mechanism and would be tidier, but it needs a live event loop inside one PowerShell
    session, and its output does not survive being piped a line at a time. A two-second diff
    of the storage stack is a few milliseconds of work, is trivially line-buffered, and fails
    in an obvious way rather than a subtle one.

    SAFE DURING A COLD-CACHE PROCEDURE. It reads the storage stack's own metadata and never
    opens a volume. In particular it does NOT probe for DCIM: a `Test-Path D:\DCIM` is an
    access, and an access is what remounts a volume that was just dismounted — the exact
    measurement hazard `examples/release-cards.rs` documents. Card identity is inferred from
    the disk model instead, which costs nothing and touches nothing.
#>
[CmdletBinding()]
param(
    [int] $IntervalSeconds = 2
)

$ErrorActionPreference = 'Stop'

# These lines are piped to another process, so they must survive the trip. Forcing UTF-8 on
# the way out is cheaper than discovering later that a separator arrived as a question mark.
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

function Snapshot {
    $out = @{}
    foreach ($d in Get-Disk -ErrorAction SilentlyContinue) {
        $letters = (Get-Partition -DiskNumber $d.Number -ErrorAction SilentlyContinue |
                    Where-Object DriveLetter | ForEach-Object { "$($_.DriveLetter):" }) -join ','
        # Keyed by serial, not by disk number: numbers are reassigned on replug — today the
        # CFexpress and the SD reader swapped 2 and 3 across one eject cycle — so a
        # number-keyed diff would report a swap as two removals and two arrivals.
        $key = if ($d.SerialNumber) { $d.SerialNumber.Trim() } else { "disk$($d.Number)" }
        $out[$key] = [pscustomobject]@{
            Number  = $d.Number
            Bus     = $d.BusType
            Name    = $d.FriendlyName
            Letters = $letters
        }
    }
    $out
}

function Describe {
    param($Key, $D)
    $where = if ($D.Letters) { $D.Letters } else { 'no letter' }
    "disk $($D.Number) | $($D.Bus) | $where | $($D.Name) [$Key]"
}

$prev = Snapshot
"watching rig state — $($prev.Count) disk(s) present, ${IntervalSeconds}s poll, metadata only"

while ($true) {
    Start-Sleep -Seconds $IntervalSeconds
    try { $now = Snapshot } catch { continue }   # a device mid-enumeration is not an error

    foreach ($k in $now.Keys) {
        if (-not $prev.ContainsKey($k)) {
            "+ ATTACHED  $(Describe $k $now[$k])"
            continue
        }

        $a = $prev[$k]; $b = $now[$k]

        # The one that matters most: a Thunderbolt enclosure falling back to its USB bridge
        # keeps working, keeps its light on, and quietly runs at a third of its rate.
        if ($a.Bus -ne $b.Bus) {
            "! BUSTYPE   $($b.Name) [$k] : $($a.Bus) -> $($b.Bus)$(if ($b.Bus -eq 'USB') { '   <-- bridged, reseat it' })"
        }
        if ($a.Letters -ne $b.Letters) {
            $from = if ($a.Letters) { $a.Letters } else { 'none' }
            $to   = if ($b.Letters) { $b.Letters } else { 'none' }
            "~ LETTERS   $($b.Name) [$k] : $from -> $to"
        }
    }

    foreach ($k in $prev.Keys) {
        if (-not $now.ContainsKey($k)) { "- DETACHED  $(Describe $k $prev[$k])" }
    }

    $prev = $now
}
