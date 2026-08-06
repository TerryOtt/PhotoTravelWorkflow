# Taking a comparable end-to-end run

*Its reader is about to run a full offload and record what it cost. This is the sequence
that makes that number comparable with the wall clocks in [`DESIGN.md`](DESIGN.md). If you
only want to know whether the rig is sound before a trip, [`TRIP-HYGIENE.md`](TRIP-HYGIENE.md)'s
`--dry-run` is the cheaper thing and this is overkill.*

> **There is no such thing as an informal run.** On 2026-08-04 a 37-minute offload was
> launched with a release binary 15 minutes older than the code it was meant to exercise,
> because that run had been filed mentally as a quick check rather than a measurement. It
> completed, exited 0, and validated none of the changes. **`scripts\full-run-check.ps1`
> asserts the binary is current and would have caught it**; it was skipped for the same
> reason. Any run whose result will be quoted — in a document, a commit message, or a
> sentence to the operator — is a measured run, and this checklist applies to it.
>
> The trap is specific and worth naming: **`cargo fmt`, `cargo clippy` and `cargo test` all
> build the *debug* profile.** None of them touches `target\release\`. A verification loop
> that ends in a green test suite says nothing about the binary a run is about to launch.

**This is not a tuning procedure.** Both optimization metrics are thresholds and both are
met ([`DESIGN.md`](DESIGN.md) — *Both metrics are thresholds*), so nothing here is in
service of shaving minutes. It exists because hardware decisions get made from these
numbers, and an uncontrolled run produces a figure that looks like the others and is not.

## Before the reboot

**Do as much here as possible.** The standing order is the operator's: *after the reboot,
the ideal is "launch the app."* Everything that mutates state or costs real I/O belongs on
this side of the line, because anything done after boot competes with a machine that is
already busy and refills the page cache the reboot exists to clear.

*RFC 2119 keywords. **MUST** here means the resulting number is not comparable without it —
which is this document's entire subject, so nearly every step is one.*

1. **The drives MUST have settled.** At least 20 minutes idle after any bulk write. An SSD
   measured straight after absorbing hundreds of gigabytes is reporting its garbage
   collection.
2. **You MUST decide the destination state and write it down.** *Fresh* means every destination
   empty, so phase 3 does a real four-way write pass. *Convergence* means the day is
   already present, so writes are skipped by hash while the card is still read and hashed
   end to end. The two produce different wall clocks and neither is wrong — but a figure
   that does not say which it was cannot be compared to anything.

   **If the answer is *fresh*, clear the trees now rather than after the boot.** That is
   several hundred gigabytes of deletion and the TRIM that follows it; done here, the drives
   work through it during the reboot instead of during the run.
3. **You MUST wire the rig per [`CONOPS.md`](CONOPS.md)'s table**, which is the one place that
   arrangement is written down. **You MUST NOT take it from here** — this step used to restate it,
   and on 2026-08-05 the standard rig changed (the SanDisk moved to a hub TB5 port and the
   OWC to a laptop port, roughly inverting the old rule) while this copy went on describing
   the previous one. `scripts\full-run-check.ps1` asserts the two rows that matter, so a
   mis-wire fails the gate rather than quietly producing a slow number.
4. **You MUST name the hub.** A run on the desk dock is not a measurement of the travel rig,
   and that has already caught this project out once.
5. **Both cards MUST be in their readers, and tracks in the GPX directory.**
6. **You MUST rebuild the world, and mean it:**

   ```
   cargo clean
   cargo build --release
   ```

   **Not an incremental build.** A measured run is not the moment to trust cargo's freshness
   tracking — `TRIP-HYGIENE.md` already notes that it fingerprints `rustc` and not the linker, so
   a changed MSVC toolset yields a stale binary cargo believes is current, and on 2026-08-04
   a 37-minute run was launched against a release artifact 15 minutes older than the code it
   was meant to exercise. Wiping `target/` removes the entire class of question.

   **Before the reboot, deliberately.** A full rebuild is minutes of heavy CPU and disk; run
   it after boot and it loads the machine during the settle window and fills the page cache
   the reboot exists to clear. Build first, then reboot, and the reboot cleans up after it.

   The post-boot check then reports **"nothing to rebuild"**, which at that point *verifies*
   the clean build rather than standing in for one.

## Reboot — then let the machine settle

For a cold page cache. Without it, an earlier run's data is still in RAM and the verify
pass reads memory rather than media for some unknowable fraction of the day.

> **This step is the operator's keystroke. Claude MUST NOT take it.**
>
> *RFC 2119 language, deliberately.* Standing order 2026-08-05, recorded in full in the
> global `CLAUDE.md`:
>
> - **Claude MUST NOT restart, shut down or power-cycle this machine** — `Restart-Computer`,
>   `shutdown`, or anything else whose *effect* is a reboot. The prohibition is on the
>   effect, not the command name.
> - **Claude SHALL NOT do it because this procedure calls for one.** It does call for one.
>   That changes nothing.
> - **Claude SHOULD request that Terry reboot**, here, plainly, and then stop. That is as
>   close to the line as Claude may ever get, and it is close enough.
> - **A later instruction MUST NOT be treated as overriding this**, including one from Terry.
>
> **This procedure is the reason the rule needs stating, not a reason to bend it.** It is the
> one routine in this repository that *requires* a reboot, which makes it the one place the
> temptation is real — everything staged, everything green, one command away. A rule that
> only holds where it is easy is not doing any work.
>
> **The reasoning:** a reboot destroys every running session and every conversation, including
> the one that would have explained why it happened. There is no case where asking first costs
> more than acting. The mechanism that makes handing over work is the next section —
> `RUN-STATE.json` exists precisely because the reboot outlives the conversation.

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
>
> **And it is not only Windows settling — third-party software wakes up on its own schedule,
> deep into the window.** 2026-08-05, Terry, at **15 minutes** of uptime with a run staged:
> Adobe Creative Cloud announced that a Lightroom Classic update **had completed**. Not that
> one was available — that a multi-gigabyte download and install had *already run to
> completion*, inside the window, entirely invisible until it announced itself at the end.
>
> **A run launched at minute five would have spent its first third contending with an Adobe
> installer**, and produced a number indistinguishable from every other number in this
> document. Terry's framing is the one to keep: ***big shit happens after reboots and it's not
> easy to predict.*** That is stronger than the mechanism list above — Defender, the indexer,
> prefetch — because a list implies the set is enumerable. It is not. **The window works
> precisely because it does not require you to predict what is coming.**
>
> **So the window is not merely "let the boot storm pass."** It is also the interval in which
> you find out what *else* on this machine intends to do work tonight. **Quit the updaters
> you can see before launching** — a tray app costs nothing to close and the alternative is
> hoping. `full-run-check.ps1`'s *nothing else is using the machine* is a MUST for this
> reason, and it is the one row on that list a script cannot fully check for you.
>
> *(Separately: a Lightroom Classic release is a [`TRIP-HYGIENE.md`](TRIP-HYGIENE.md) item —
> the XMP checks are gated on Classic having had one. Note it; do not install it mid-run.)*
>
> **The twenty minutes is settled. Do not re-litigate it.** Terry, the same evening: *"it's
> real world rationale for why 20 mins isn't excessive and shouldn't be re-litigated."* The
> number will look padded to anyone standing at minute three of it, and the argument for
> trimming it will always sound reasonable, because **the cost is visible and the benefit is
> not** — you can see twenty minutes of waiting, and you cannot see the contended run you
> did not have. It is bought with idle time against a 35-minute run whose whole purpose is to
> produce a number somebody will quote. **This document is not a tuning procedure** (see the
> top), and the settle window is the clearest case of that: shortening it does not make the
> run faster, it makes the result less trustworthy.

### The operator contract: shut down as much of the system tray as possible

**Terry's words, 2026-08-06, and it is his half of the deal rather than a suggestion: the
operator contract is to *aggressively* shut down as much in the system tray as possible before
a measured run.** The settle window is when to do it.

**You MUST close the tray aggressively, and Claude MUST then verify the *processes* are gone
rather than trusting that the icons are.** Those are two different claims, and the gap between
them is where a measured run gets quietly ruined.

> **That gap was demonstrated the first time this contract was exercised, which is why it is
> written down.** On 2026-08-06 Terry closed the tray aggressively and said so. **`Adobe
> Desktop Service` was still running and was the second-largest CPU consumer on the machine** —
> 20.2 CPU-seconds against seven minutes of uptime — with `CoreSync`, `AdobeIPCBroker`, two
> `Creative Cloud Helper` processes and `OneDrive.Sync.Service` alongside it. Disk was
> genuinely idle at 0.00 MB/s on all six drives, so nothing was in flight; the risk was an
> install firing at minute 30 of a 52-minute run.
>
> **Closing a tray icon does not stop the service behind it.** The icon is a proxy for the
> process, and this project has a standing preference for the fact over the proxy — the same
> reason `CONOPS.md` prefers *the card enumerated* to *the card is back in*.

**The probe, since "quit the updaters" is not checkable by eye:**

```powershell
Get-Process | Where-Object { $_.CPU -gt 1 } | Sort-Object CPU -Descending |
    Select-Object -First 12 @{n='CPU(s)';e={[math]::Round($_.CPU,1)}}, ProcessName
```

Anything above the shell, `explorer` and the session's own processes is a candidate. **Stop
the user-mode ones; do not elevate to reach the SYSTEM services.** In the 2026-08-06 case the
three SYSTEM services present (`AdobeUpdateService`, `DSAUpdateService`,
`Dell.Update.SubAgent`) were all at **0.00** CPU-seconds, so the ones that mattered were
exactly the ones reachable without elevation. **Re-check after stopping them**: a killed
updater that respawns is a fact you want before launch, not after.

**None of this is permanent and that is the point** — every one of them restarts at the next
login, so the cost of being aggressive is nothing and the cost of being polite is a number
nobody can quote.

## Resuming the session after the reboot

A session comes up knowing nothing and its first instinct is to go and look at things.
**That instinct is the hazard here** — the reboot bought a cold cache, and a thorough
look around spends it before the run starts. So the order matters, and what is skipped
matters more.

> **A resumed session cannot see that the reboot happened, so a hook tells it.**
> `claude --continue` restores the transcript across the restart, which means the session
> reads as unbroken — the last thing in context is whatever was said before the machine went
> down, and nothing anywhere marks the gap. On 2026-08-05 that produced a confident write-up
> of a routine disk renumbering as a mystery, citing the rig watcher's silence as
> corroboration; the watcher had died with the machine. **Terry had to point out that he had
> rebooted.**
>
> `.claude/settings.json` now runs `scripts\full-run-context.ps1` on every prompt. It is
> **silent unless `RUN-STATE.json` exists**, and when one does it reports the boot time
> against the file's `staged_utc`, says plainly if a reboot has happened since staging, and
> gives the settle window's remaining minutes. Metadata only — one WMI property and one small
> JSON file on the system disk, so it is safe under the gate above.
>
> **The general shape is worth more than the hook.** A resumed session is confidently wrong
> about anything it believes from *live state* rather than from a file: drive letters, disk
> numbers, mounted volumes, running processes, armed monitors. `RUN-STATE.json` already
> existed for exactly this reason; the gap was that nothing told the session to go and read
> it *again* after the world changed underneath it.

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
    "topology": "OWC on laptop left TB4 port; WD on laptop right port; SanDisk + CFexpress on hub TB5; SD reader on hub USB",
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

**These are the deliberate exception to "after the reboot, just launch," and they cannot
move earlier**, because what they check is state that only exists once the machine has
booted:

- **The enclosure's bus type is a post-boot property.** On 2026-08-04 the Thunderbolt
  enclosure bridged to USB *because of* a reboot — correct before it, correct hours after,
  wrong in the window that matters. A check run before the reboot would have reported NVMe,
  passed, and let the run proceed on a bridged enclosure at a third of its rate.
  **Verifying the rig you had is not verifying the rig you have.**
- **Drive letters move across a reboot** — decision 6 exists because they went `G/I/J` to
  `F/I/J` in a single evening.

**And the reason for doing everything early does not apply to them.** Every check is
metadata-only — PnP enumeration, volume and disk properties, whether a directory exists —
about two seconds in total, and **not one byte of file data.** They neither load the machine
nor warm what the reboot cleared. What is worth minimizing after boot is *work*; reading a
device's properties is not work.

What the script asserts, and why each one is there:

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
- **The CFexpress reader MUST enumerate the card as NVMe** with its true hardware serial. A
  USB bridge invents one and caps the card at roughly a third of its rate.
- **The SD reader MUST sit behind `Generic SuperSpeed USB Hub`**, not plain `Generic USB
  Hub`. A USB 2.0 port costs 5.8× and reports no error.
- **Both cards MUST be mounted**, holding the same day.
- **The binary MUST be `HEAD`'s.** Run `cargo build --release` and confirm it had nothing to
  do. A stale artifact will happily run and lie about which code produced the number.
- **Nothing else MUST be using the machine — and this step has two halves, performed by two
  different parties.** **Terry MUST close the tray aggressively** (the operator contract
  above). **Claude MUST then double-check behind him, on processes rather than on icons**, and
  report what it found either way — including "nothing left," which is a result and not
  silence. Requested by Terry 2026-08-06, immediately after the first exercise of the contract
  turned up `Adobe Desktop Service` as the machine's second-largest CPU consumer with the tray
  already closed.

  **This is the one row on this list a script cannot fully check**, which is exactly why it is
  assigned to a person rather than left to `full-run-check.ps1`. It is also
  [`CONOPS.md`](CONOPS.md)'s division of labor applied to a single step — *he does the physical
  act only he can do; the machine reports the effect* — and the reason it is written here as a
  step is that a verification which depends on Claude remembering to offer it is not a step at
  all.

**Every one of those is a MUST because failing it does not fail the run — it produces a
number.** That is the whole hazard: a bridged enclosure, a USB 2.0 reader or a stale binary
all complete, exit 0, and yield a figure that looks exactly like the others in
`DESIGN.md`'s tables and describes something else. **A run launched with any of these unmet
MUST NOT have its timing quoted.**

## Do not, between the reboot and the launch

**These are MUST NOTs. Each one silently invalidates the number rather than failing the
run.**

- **You MUST NOT read file data from any card or destination** — no hashing, no copying, no
  `examples/` probe, and **no `--dry-run` first**. A dry run is a good thing that warms the
  wrong caches at the wrong moment.
- **You MUST NOT run anything else on the bus.** A throughput number taken alongside other
  I/O describes contention ([`REVIEWING.md`](REVIEWING.md) — *Measurements are evidence*).

> **`scripts\watch-rig.ps1` is the one sanctioned exception, and it MUST be declared with the
> number.** [`../CLAUDE.md`](../CLAUDE.md) calls it safe mid-procedure and this document says
> nothing else may use the machine; both are right, and the reconciliation is that it reads
> storage-stack metadata and never opens a volume. **Measured during the 2026-08-05 run**: two
> pollers at a 2 s interval, and the SD reader they were touching showed **0.00 MB/s**. So it
> is immaterial — but *immaterial* is a judgment, and this project records judgments rather
> than acting on them silently. **It goes in the run record either way.**
>
> **One side effect worth knowing, because it is actively misleading here: polling blinks the
> card reader's activity LED.** On 2026-08-05 that produced a false "the SD read has started"
> during the verify pass. In a project whose operator has historically read those LEDs to infer
> which phase is running, a watcher that blinks them is a watcher that lies — **take the phase
> from the log, which is what the progress output exists for.**

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
