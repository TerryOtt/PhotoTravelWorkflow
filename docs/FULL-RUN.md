# Taking a comparable end-to-end run

*Its reader is about to run a full offload and record what it cost. This is the sequence
that makes that number comparable with the wall clocks in [`DESIGN.md`](DESIGN.md). If you
only want to know whether the rig is sound before a trip, [`UPDATING.md`](UPDATING.md)'s
`--dry-run` is the cheaper thing and this is overkill.*

**This is not a tuning procedure.** Both optimization metrics are thresholds and both are
met ([`DESIGN.md`](DESIGN.md) — *Both metrics are thresholds*), so nothing here is in
service of shaving minutes. It exists because hardware decisions get made from these
numbers, and an uncontrolled run produces a figure that looks like the others and is not.

## Before the reboot

1. **Settle the drives.** At least 20 minutes idle after any bulk write. An SSD measured
   straight after absorbing hundreds of gigabytes is reporting its garbage collection.
2. **Decide the destination state, and write it down.** *Fresh* means every destination
   empty, so phase 3 does a real four-way write pass. *Convergence* means the day is
   already present, so writes are skipped by hash while the card is still read and hashed
   end to end. The two produce different wall clocks and neither is wrong — but a figure
   that does not say which it was cannot be compared to anything.
3. **Wire the rig per [`CONOPS.md`](CONOPS.md)** — both archive USB SSDs into the laptop's
   own ports, the Thunderbolt enclosure and both card readers on the hub.
4. **Name the hub.** A run on the desk dock is not a measurement of the travel rig, and
   that has already caught this project out once.
5. **Both cards in their readers, tracks in the GPX directory.**

## Reboot — then let the machine settle

For a cold page cache. Without it, an earlier run's data is still in RAM and the verify
pass reads memory rather than media for some unknowable fraction of the day.

> **Then wait, because the reboot this procedure requires puts you in the worst measurement
> window it has.** Established 2026-08-04, when three unrelated oddities in one evening
> turned out to share a cause: the Thunderbolt enclosure bridged to USB instead of PCIe at
> 11–15 minutes after boot and never once in ten cycles at 103–112 minutes; the SD card read
> 168 MB/s at 17 minutes and 212 MB/s flat at 117; and phase 4 reported five transient read
> errors from that same card between 26 and 52 minutes. **Every anomaly was inside the first
> hour. Every clean reading was two hours in.**
>
> A freshly booted Windows is a busy one — Defender catching up, the indexer, prefetch,
> services still starting — and this project's own standing order is that a number taken
> while something else is using the bus is not a measurement.
>
> **Waiting is nearly free, which is what makes this an easy fix rather than a tradeoff.**
> Windows settling reads its own files, not the 201 GB of raws on the cards and
> destinations, so the cache the reboot just cleared stays cleared. **Give it 20 minutes,
> and re-run `full-run-check.ps1` after the wait rather than before** — the enclosure's bus
> type is exactly the thing that changes during it.

## Resuming the session after the reboot

A session comes up knowing nothing and its first instinct is to go and look at things.
**That instinct is the hazard here** — the reboot bought a cold cache, and a thorough
look around spends it before the run starts. So the order matters, and what is skipped
matters more.

**Safe, and required first** — small text files on the system disk, nothing on a card or an
archive destination:

1. **[`DESIGN.md`](DESIGN.md), then every file in the memory directory, in full** — not
   just the index. `CLAUDE.md` requires this of every session; it is not extra work for a
   measured run, and it is what supplies the expected serials, link types and per-device
   rates the checks below compare against.
2. **Repository state** — `git status`, `git log`, and the config the run will use.

**Safe, and the point of the exercise** — metadata only: PnP enumeration, disk and volume
properties, a directory listing to confirm what a destination already holds. None of it
reads file data, so none of it warms what the reboot just cleared. That is the section
below.

**Not safe, however tempting:**

- **Reading photo data anywhere** — a CR3, a hash, a byte count taken by reading files. A
  directory listing is metadata; opening what it lists is not.
- **Walking the archive trees deeply to characterize them.** Counting frames per date
  folder across four destinations is a metadata sweep, so it does not warm file data — but
  it does warm the filesystem metadata the run is about to walk itself, and it is rarely
  worth what it tells you. **The tool prints what it found in pre-flight.** Take the
  answer from the run.
- **A `--dry-run` first**, or any `examples/` probe. Both are good things at the wrong
  moment.

**If a check is skipped or a rule is broken, say so and carry it on the number** — tonight's
run has a metadata-walk caveat on it for exactly this reason. A caveat costs a sentence; a
number nobody can reproduce costs the afternoon that produced it.

## `RUN-STATE.json` — how the constraint survives the reboot

**The rule this file exists to enforce cannot live in a conversation, because the reboot is
what destroys conversations.** A session that comes back without context does not know it is
mid-procedure, and does not announce that it doesn't — it just starts confidently looking
around. So the state goes on disk, at the repository root, where
[`CLAUDE.md`](../CLAUDE.md)'s first rule makes every session find it before its first tool
call.

Write it **before** the reboot. Delete it once the run is recorded.

```json
{
  "purpose": "cold-cache end-to-end run",
  "step": "rebooted; rig checks not yet run",
  "prohibited": "reading file data from any card or destination — see docs/FULL-RUN.md",
  "established": {
    "hub": "CalDigit Element 5",
    "topology": "SanDisk+WD on laptop ports, OWC and both readers on the hub",
    "destination_state": "convergence on laptop/SanDisk/WD, OWC empty",
    "binary": "8dd23e5, cargo build --release was a no-op"
  }
}
```

**`established` is the part that earns its keep**, and it is why this is not merely a step
counter: a step number goes stale the moment the procedure changes, while the facts a
resuming session would otherwise go and re-derive — by looking around, which is the
prohibited act — stay true. Anything already confirmed goes here so nobody confirms it
twice.

Resuming the *session* rather than starting a fresh one (`claude --continue` in the project
directory) is worth doing and is not a substitute: a resumed session may have been
compacted, so a detail stated only in chat can still be gone. The file is what does not
depend on that.

## After boot, before launching

**Run `scripts\full-run-check.ps1`.** It performs every check below and exits non-zero if
any fails, so the run can be gated on it. Prefer it to checking by hand — the checks were
improvised at the keyboard once, and improvising is how a session ends up walking the
archive trees it was told to leave alone. A fixed set of checks removes the decision.

Every check is metadata-only — PnP enumeration, volume and disk properties, whether a
directory exists. **None of it reads file data, so none of it warms what the reboot just
cleared.** What the script asserts, and why each one is there:

- **The Thunderbolt enclosure must be on PCIe, not bridged to USB.** `Get-Disk` must show
  the bare NVMe inside it with the serial `config.json` carries, `BusType` **NVMe**. If it
  names the *enclosure* with a different serial and `BusType USB`, the PCIe tunnel did not
  come up — reseat it and re-check. This is silent: the link trains, the router enumerates,
  the volume mounts, every file reads back.

  **This check earns its place here specifically, and the reason is the reboot.** Measured
  2026-08-04: the fallback happens in the *minutes after a boot* and not afterwards — ten
  controlled plug cycles hours later came up PCIe every time, with and without the other
  Thunderbolt device attached, while three of four attempts minutes after boot bridged to
  USB. **The one procedure that reliably provokes it is the one in this document**, since a
  cold-cache run begins by rebooting. Reseat until it reports NVMe; it clears on its own
  once the machine settles.
- **The CFexpress reader must enumerate the card as NVMe** with its true hardware serial. A
  USB bridge invents one and caps the card at roughly a third of its rate.
- **The SD reader must sit behind `Generic SuperSpeed USB Hub`**, not plain `Generic USB
  Hub`. A USB 2.0 port costs 5.8× and reports no error.
- **Both cards mounted**, holding the same day.
- **The binary is `HEAD`'s.** Run `cargo build --release` and confirm it had nothing to do.
  A stale artifact will happily run and lie about which code produced the number.
- **Nothing else is using the machine.**

## Do not, between the reboot and the launch

- **Read file data from any card or destination** — no hashing, no copying, no
  `examples/` probe, and **no `--dry-run` first**. A dry run is a good thing that warms the
  wrong caches at the wrong moment.
- **Run anything else on the bus.** A throughput number taken alongside other I/O describes
  contention ([`REVIEWING.md`](REVIEWING.md) — *Measurements are evidence*).

## Launch, and what to record with the number

Run the bare command and note the UTC start. When it finishes, the figure is worth nothing
on its own — record it with the conditions that make it comparable:

- which hub, and the full topology: what is on the laptop's own ports, what is on the hub,
  and which link each device negotiated
- fresh or convergence, and which destinations were already populated
- which cards were the source and the corroborator
- exit code, the LANDED time, the per-phase timings and the per-destination rates
- **anything that touched the bus that should not have**, including metadata walks — say it
  rather than hoping it was too small to matter

## Why the list is this shape

Each entry is here because its absence has already produced a wrong number in this project,
or would have:

| Step | What it prevents |
|---|---|
| Settle the drives | Post-write garbage collection reported as device speed |
| Reboot | The verify pass reading RAM instead of media |
| Record fresh-versus-convergence | Two runs that did different amounts of work compared as though they had not |
| Name the hub | A figure describing a rig that never leaves the house |
| Check the enclosure's bus type | A drive silently bridged to USB at a third of its rate |
| Check the reader's link generation | A sound card mistaken for a dying one |
| Confirm the binary | A number attributed to code that did not produce it |
| No probes, no dry run first | Warmed caches, and contention reported as throughput |

**And the standing order that outranks all of it:** if you did not run it, say the number is
an estimate; if something else was on the bus, say so or take it again.
