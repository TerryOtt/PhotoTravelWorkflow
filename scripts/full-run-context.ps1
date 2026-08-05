# Tell the session what the machine did while it was not looking.
#
# **This exists because a reboot is invisible from inside a conversation.** `claude --continue`
# restores the transcript across a restart, so the session reads as unbroken: the last thing in
# context is whatever was said before the machine went down, and nothing anywhere says a boot
# happened. On 2026-08-05 that produced a confident write-up of an ordinary disk renumbering as
# a mystery — "same session, nothing unplugged" — with the rig watcher's silence offered as
# corroboration. The watcher had died with the machine. Terry had to point out that he had
# rebooted, which is not a thing he should ever have to do.
#
# `docs/FULL-RUN.md` is the only routine here that *requires* a reboot, so it is the routine
# where this blind spot is guaranteed rather than merely possible.
#
# **It stays silent unless RUN-STATE.json exists.** That file marks a staged or in-flight
# measured run, and outside one this information is noise — a hook that speaks on every prompt
# in every context is a hook that gets tuned out, which is the failure mode this whole project
# keeps arguing about. Emitting nothing is a valid hook response.
#
# Metadata only: one WMI property and one small JSON file on the system disk. It reads nothing
# from a card or a destination, so it is safe under the FULL-RUN gate that forbids exactly that.

$ErrorActionPreference = 'Stop'

$repo = Split-Path -Parent $PSScriptRoot
$statePath = Join-Path $repo 'RUN-STATE.json'

# No staged run, nothing to say. The common case, and it MUST stay silent.
if (-not (Test-Path $statePath)) { exit 0 }

$boot = (Get-CimInstance Win32_OperatingSystem).LastBootUpTime
$bootUtc = $boot.ToUniversalTime()
$uptimeMin = ((Get-Date) - $boot).TotalMinutes

$lines = @()

# The headline. A boot after staging is the thing the session cannot otherwise know, so it
# leads, and it says what to do about it rather than only what happened.
$staged = $null
try { $staged = (Get-Content $statePath -Raw | ConvertFrom-Json).staged_utc } catch { }

if ($staged) {
    $stagedUtc = [datetime]::Parse($staged, [Globalization.CultureInfo]::InvariantCulture,
                                   [Globalization.DateTimeStyles]::AdjustToUniversal -bor
                                   [Globalization.DateTimeStyles]::AssumeUniversal)
    if ($bootUtc -gt $stagedUtc) {
        $lines += "** THE MACHINE REBOOTED AFTER THIS RUN WAS STAGED. **"
        $lines += "   Staged {0}, booted {1}. The conversation does not show this - it was" -f $stagedUtc.ToString('u'), $bootUtc.ToString('u')
        $lines += "   restored across the restart. Anything you believe about live state from"
        $lines += "   before that boot is stale: drive letters, disk numbers, mounted volumes,"
        $lines += "   background processes, and every Monitor armed earlier (they died with it)."
    }
    else {
        $lines += "No reboot since staging (staged {0}, booted {1})." -f $stagedUtc.ToString('u'), $bootUtc.ToString('u')
        $lines += "docs/FULL-RUN.md requires one before the run - it has not happened yet."
    }
}
else {
    $lines += "RUN-STATE.json has no staged_utc, so a reboot cannot be detected. Last boot {0}." -f $bootUtc.ToString('u')
}

# The settle window. FULL-RUN.md's 20 minutes is not padding: the enclosure bridges to USB and
# the SD reader reads slow inside it, both measured, both self-clearing.
$lines += ""
$lines += "Uptime {0:N1} min. Settle window (20 min): {1}" -f $uptimeMin, $(
    if ($uptimeMin -ge 20) { "MET" } else { "NOT MET - {0:N1} min remaining, do not launch" -f (20 - $uptimeMin) })
$lines += "A measured run MUST NOT be launched before the window is met, and full-run-check.ps1"
$lines += "MUST be re-run after the wait - the enclosure's BusType is what changes during it."

@{
    hookSpecificOutput = @{
        hookEventName    = 'UserPromptSubmit'
        additionalContext = "[FULL-RUN in progress - RUN-STATE.json present]`n" + ($lines -join "`n")
    }
} | ConvertTo-Json -Depth 5 -Compress
