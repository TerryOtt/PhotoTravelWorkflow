# Trip hygiene

*Everything that gets updated, checked and rehearsed **at home** so that nothing surprises
you in a hotel room. **"Trip hygiene" is this project's name for this routine** — the one
term, used in docs, commit messages, memory and conversation alike. Do not coin a synonym
for it; a routine with three names is three habits nobody performs completely.*

**Cadence: once per photo trip, before you leave.** Not on the road. The whole point of
doing it on a schedule is that a surprise — a crate that changed behavior, a toolchain that
needs reinstalling, a config that no longer matches the hardware — surfaces while you are
at home with the rig and a real network connection, rather than in a hotel room with two
card readers and 2,000 photographs waiting to be offloaded.

If a trip is imminent and the update looks at all interesting, **skip it and travel with
the binary you have.** A version that is four point releases behind and verified is worth
more than a current one you have not run against real cards.

## The timeline, and the one hard line in it

Most of trip hygiene can happen at any point before departure. **One item cannot, and it is
a bright line rather than a judgment call:**

| When | What |
|---|---|
| **T-30 days and earlier — never after** | **Device firmware:** hub, enclosure, card readers, **and the camera body** |
| Any time — **and the later the better** | **The camera clock: set to UTC, then *verified* by taking a frame.** Not frozen; see below |
| Any time before you leave | Dependencies, toolchain, MSVC Build Tools, workflow action pins |
| Any time before you leave | Lightroom XMP checks, if Classic had a major release |
| **Last, after everything above** | `offload --dry-run` against the real rig |

The dry run is last on purpose and for the same reason throughout: it is the rehearsal, so
anything changed after it is untested. **Change, then rehearse — never the reverse.**

## Also before you leave

**Run a full dry run against the actual rig.** Plug in both readers and all three SSDs,
and:

```
offload --dry-run
```

This is the closest thing to a fixture check this tool has, and it costs seconds. It
resolves every destination by disk serial and volume GUID, asserts they are four distinct
devices, benchmarks both card readers, parses the GPX directory, and prints what it would
write. A drive that was reformatted since the last trip, a reader that now enumerates
differently, or a config entry pointing at an SSD that is in a drawer all surface here
rather than at 11pm in a hotel.

**Has Lightroom Classic had a major version since the last trip?** If so, run the two
checks in RawGeotag's `docs/LIGHTROOM-XMP.md` — the XMP engine lives there and that is
where its verification stays. Same reasoning as this file, different trigger: find out that
Lightroom moved while you are at home. Dot releases do not warrant it.

### Device firmware — T-30 days or not at all

> **Standing order, and a bright line: firmware is updated at T-30 days or earlier, never
> inside thirty days of departure.** Not "prefer not to". Inside the window the answer is
> no, whatever has shipped and however tempting the changelog.

**The freeze is sized by replacement lead time, not by how risky the flash is.** That is
the whole reasoning and it is why the number is thirty rather than seven: if a flash bricks
an enclosure, the recovery is not a rollback, it is **ordering new hardware and waiting for
it to arrive** — and then running trip hygiene again against the replacement. Thirty days
buys the order, the delivery and the re-verification with room to spare. Seven buys panic.

**The point is not avoiding the drama, it is choosing when the drama happens.** A bricked
enclosure discovered at T-40 is an errand. The identical failure at T-5 is a trip that
leaves with three copies instead of four, or does not leave at all. The hardware does not
know the difference; the calendar is the entire variable, so the calendar is what gets
governed.

**Which cuts the other way too, and is why this reads as a schedule rather than a
prohibition: at T-30 and earlier, do it.** Firmware is the one layer of this rig that
nothing else in this document can see, and skipping it indefinitely is how a device sits
years behind. The freeze exists so the update has a home, not so it never happens.

**Only ever at home** — the rest of this section holds regardless of the calendar.

The rig is a Thunderbolt chain, and a chain is firmware at both ends: the hub, the
enclosure, the card readers, and the laptop's own host router. **`cargo outdated` cannot
see any of it, rustup cannot either, and nothing else in this process would notice a device
being years behind.**

**It is not hypothetical here.** On 2026-08-05 the OWC enclosure came up bridged to USB
instead of carrying its PCIe tunnel — every file readable, activity light on, `BusType` the
only tell — which is precisely the class of bug router firmware addresses. It is cheap to
detect (`scripts\full-run-check.ps1` asserts it) and a cable reseat clears it, which makes
it a nuisance at home and a silent 3× throughput loss in a hotel room where nobody thinks
to look.

Read what is installed, and write the numbers down before changing any of them — the
before-state is only free to capture once:

```powershell
Get-PnpDevice -PresentOnly |
    Where-Object { $_.FriendlyName -match 'USB4 Router|Thunderbolt 3.*Router' } |
    ForEach-Object {
        '{0,-8} {1}' -f (Get-PnpDeviceProperty -InstanceId $_.InstanceId `
            -KeyName 'DEVPKEY_Device_FirmwareVersion').Data, $_.FriendlyName
    }
```

**One trap that will send you hunting the wrong thing: an enclosure that has fallen back to
USB does not enumerate as a router at all.** It has no firmware property to read and simply
appears absent from that list — so confirm `BusType` reads **NVMe** before concluding
anything about its firmware.

**Then dry-run, never the reverse** — the timeline above, applied to this item
specifically. A rehearsal performed before the flash validated firmware the rig is no
longer carrying, which is this file's opening rule with more force behind it: *travel with
what you have verified, not with what is current.* The T-30 line is what guarantees there
is room left to re-verify at all; inside it there would not be, which is the second reason
the freeze exists.

### The camera body is a trip device too

**Its firmware sits under the same T-30 line**, and for the same reason: a body that will
not boot is replaced, not rolled back, and the fleet is exactly one camera
([`CONOPS.md`](CONOPS.md)). There is no spare to shoot with.

**And a body flash can reset the clock and the timezone, which is why the clock check comes
after it and never before.** Verifying the clock and then flashing over it proves nothing —
the same *change, then rehearse* ordering that governs everything else here.

**The clock check itself is explicitly *not* frozen at T-30, and the distinction is the
whole logic of the freeze.** What T-30 governs is *irreversibility*: a flash that goes wrong
is replaced hardware and a delivery window. Setting a menu item and shooting one frame is
reversible in ten seconds, costs nothing, and cannot brick anything — so it carries none of
what the freeze exists to hold back. **Do it as late as you like, and preferably late**: it
is the item most likely to be quietly undone between now and the airport, by a flash, a
battery pull, or a menu reset. `CONOPS.md` already asks for it again at trip start and after
every zone crossing, for exactly that reason.

**Read the freeze as scoped to the risky thing, never to the checklist it appears on.**
Anything else here that is cheap and reversible is likewise fine inside thirty days; the
firmware rows are the only ones with a date on them.

#### Set UTC — which on an R5 means London with DST **off**

**The R5 offers no UTC timezone.** London is the stand-in, and that is the whole trap:
London with daylight saving left on is **BST, `+01:00`**, all summer. The setting *looks*
right in the menu and is wrong in the file.

#### Then verify it by taking a picture, because the menu is not evidence

**The menu shows what was set. The frame shows what the camera writes.** Those are different
claims, and every timezone bug this project has seen lives in the gap between them. So:

1. **Note the true UTC** at the moment you press the shutter — `Get-Date -AsUTC` on the
   laptop, or any clock you trust.
2. **Take one frame.**
3. **Hand it to Claude**, who reads its EXIF and checks *both* properties below.

| What is checked | Reading it wrong means | Caught later? |
|---|---|---|
| `OffsetTimeOriginal` is `+00:00`, not `+01:00` | DST is on — London is running as BST | **Yes** — the tool self-corrects and the report flags it |
| Wall time **minus** that offset equals the UTC you noted | **The clock itself is wrong** | **No. Nothing can.** |

**The second row is the one that matters, and it is the reason this check exists at all.**
Decision 23 is explicit that no timezone *setting* can misfile a photo — the offset is
recorded, the arithmetic self-corrects, and a BST frame still lands in its true UTC folder.
A wrong *clock* is the opposite: it derives a wrong UTC from honest arithmetic, so the date
folders and every geotag shift together with no error anywhere, because the metadata is
lying rather than broken. **A frame taken at a known instant is the only thing that catches
it, and there is no second chance after the trip.**

The failure worth naming, because it is how a merely-cosmetic problem becomes the fatal
one: noticing `+01:00` and "fixing" it by winding the *clock* back an hour instead of
turning DST off. That trades a self-correcting offset for a wrong absolute instant — it
looks like a fix, the menu now reads `+00:00`, and every frame on the trip is an hour out
in a way nothing downstream can detect.

**The zero-tooling version of the important half**, if Claude is not to hand: put the card
in and run `offload --dry-run`. Output names are `HHMMZ_NNNN.CR3` derived from UTC, so the
printed filename *is* the camera's derived UTC — compare it against the instant you noted.
That does not show the offset, so it does not distinguish the two rows above, but it
catches the row that cannot be caught later.

**The asymmetry that earns firmware its own rule:** a bad crate bump fails loudly at
compile time and reverts with `git checkout Cargo.lock`. A bad flash can brick an enclosure
holding one of the four archive copies. So do one device at a time, read the vendor's notes
rather than clicking through, and never with a restore or an offload in flight.

## The short version

```
cargo outdated                   # what is behind, and whether cargo can reach it
cargo update                     # take everything semver-compatible
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
git add Cargo.lock && git commit
```

Then read the rest of this file, because two of those steps have a trap in them.

`cargo outdated` is not part of cargo. Install it once with `cargo install cargo-outdated`.

## Step 1 — see what is behind, and read the columns properly

```
Name                Project  Compat  Latest  Kind    Platform
----                -------  ------  ------  ----    --------
clap                4.6.4    4.6.5   4.6.5   Normal  ---
clap->clap_builder  4.6.2    4.6.5   4.6.5   Normal  ---
```

| Column | Means |
|---|---|
| `Project` | what your `Cargo.lock` is pinned to now |
| `Compat` | the newest version **`cargo update` can reach** without editing `Cargo.toml` |
| `Latest` | the newest version on crates.io, ignoring your version requirement |

**The one thing to look at is whether `Compat` and `Latest` agree.** Where they do, the
update is free: `cargo update` takes it and the manifest never changes. Where `Latest` is
ahead of `Compat`, cargo *cannot* get you there and the row needs a hand edit; see the next
step.

Rows spelled `a->b` are transitive: `gpx->time` is the `time` that `gpx` pulls in.

## Step 2 — the `0.x` ceiling, which is the trap

Cargo's rule for pre-1.0 crates is that the **minor** position is where breaking changes
live. So `indicatif = "0.18"` means `>=0.18.0, <0.19.0` and **can never resolve to 0.19**,
no matter how many times you run `cargo update`. Cargo will tell you "Locking 0 packages to
latest compatible versions" — which is true, reassuring, and entirely consistent with being
three minor releases behind.

**A clean `cargo update` is not evidence of being current.** This is not hypothetical: in
RawGeotag `indicatif` sat pinned at `"0.17"` while 0.18 had already shipped six patch
releases, and it stayed that way until a human noticed.

`cargo outdated` is what catches it, because its `Latest` column ignores your requirement.
When a row shows `Latest` ahead of `Compat`:

1. Edit the version string in `Cargo.toml` by hand.
2. Read that crate's changelog — a `0.x` minor bump is a *breaking* release and is allowed
   to have moved anything.
3. Run the full verification below. This is the case where it earns its keep.

**Where the risk sits.** The `1.x` deps are self-correcting — `"1"` keeps picking up 1.x
forever. The exposure concentrates in the pre-1.0 crates, which as of decision 29 are:

| Crate | Pinned | What a silent minor bump would cost you |
|---|---|---|
| `gpx` | 0.10 | how a track parses — the thing `--dry-run` against the rig would catch and `cargo test` would not |
| `chrono` | 0.4 | the program's own instant and duration types; a break here touches every date folder |
| `time` | 0.3 | `gpx`'s public type at one boundary; moves when `gpx` does |
| `sha2` | 0.11 | the hash in every manifest. A backend change is a throughput question, not a correctness one — `cargo run --release --example hash-rate` answers it |
| `windows` | 0.62 | the storage-identity layer, eject and unbuffered I/O — all of it |
| `windows-registry` | 0.6 | the Defender check only, which already warns rather than fails |
| `indicatif` | 0.18 | the progress bars. Cosmetic, and the one that actually went stale in RawGeotag |
| `console` | 0.16 | the verdict's styling. Cosmetic |

`nom-exif` reads `"3.6"` and looks pre-1.0 at a glance; it is not, so Cargo's minor rule
does not apply to it and `cargo update` reaches 3.x freely.

### A declared-but-unused workspace dependency is invisible too

**Found 2026-08-05.** Three crates sit in `[workspace.dependencies]` and are imported by no
member: `indicatif`, `console` and `windows-registry`. Nothing references them, so they
never enter the dependency graph, **they are not in `Cargo.lock` at all, and
`cargo outdated` does not check them** — it reports on what resolved, and these never did.

That is a direct consequence of a rule this workspace keeps on purpose: *a member's own
manifest lists only what its code imports today, so a manifest never claims a dependency
nothing uses.* Worth keeping — but its cost is this hole, and the hole lands on exactly the
crates the table above calls most at risk. All three are pre-1.0, and one of them,
`indicatif`, is the crate that actually went stale in RawGeotag.

The reason all three are unused is that their features are unbuilt: `indicatif` and
`console` belong to decision 14's full report, `windows-registry` to decision 9's Defender
check. **They become visible the day their code is written**, which is the good news — this
is a gap that closes itself, and until then it has to be checked by hand.

```
cargo search indicatif --limit 1
cargo search console --limit 1
cargo search windows-registry --limit 1
```

**Compare the minor, not the whole version** — that is the same `0.x` rule one section up,
and it is easy to get backwards when reading this output. `"0.18"` against a current 0.18.6
is *fine*, because the requirement reaches it. `"0.17"` against a current 0.18 is the trap.
A checker that flags the first as behind produces a false alarm on a workspace with nothing
to do, which is how a real one later gets ignored.

*Checked 2026-08-05: `indicatif` 0.18 vs 0.18.6, `console` 0.16 vs 0.16.4,
`windows-registry` 0.6 vs 0.6.1 — every declared minor is the current minor, so all three
are current and nothing needed doing. The blind spot was real; the alarm was not.*

### The workflow pins dependencies too, and `cargo outdated` cannot see them

`.github/workflows/ci.yml` pins its actions by major tag, which never moves on its own and
which nothing in this file's process would otherwise check. GitHub announces a stale one as
a *run annotation* rather than a failure, so it is invisible unless someone opens a green
run and reads it:

```
gh api repos/actions/checkout/releases/latest --jq .tag_name
```

**Ask the API, do not recall the number** — the same rule as the crates above, and it has
already bitten once in exactly the way that section warns about: a v5 was recommended from
memory on the day v7.0.1 was current. The usual reason a bump is needed is the runner
dropping a Node version, so check what the action runs on (`using:` in its `action.yml`)
rather than assuming the tag is cosmetic.

### The toolchain is not a dependency either

`cargo outdated` sees crates. It does not see rustup, the stable toolchain, or the MSVC
linker every build here goes through, and nothing else in this process would notice any of
them being behind.

rustup answers for both itself and the toolchain in one line:

```
rustup check
```

The MSVC side has no such command, and the obvious substitute is the misleading one.
**`winget upgrade --id Microsoft.VisualStudio.BuildTools` answers from winget's own source,
which lags the Visual Studio release channel** — so "No available upgrade found" is
consistent with being behind, in the same way a clean `cargo update` is. Ask the channel
manifest instead, which is what the VS Installer itself serves updates from, and compare it
against what is installed:

```powershell
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$have = (& $vswhere -products * -format json | ConvertFrom-Json).installationVersion
$want = (Invoke-RestMethod https://aka.ms/vs/18/stable/channel).channelItems |
        Where-Object id -eq 'Microsoft.VisualStudio.Product.BuildTools' |
        Select-Object -ExpandProperty version
"installed $have / channel $want"
```

**Compare those two fields specifically.** The manifest also carries
`info.productDisplayVersion`, which is the same release written the marketing way —
`18.8.2` where `installationVersion` says `18.8.12023.21` — and is the form winget reports.
Comparing one against the other reads as a mismatch on a machine that is perfectly current.
The `18` in the URL is the VS major version, and is the one thing here that has to be
edited by hand rather than asked for.

## Step 3 — verify, and mean it

```
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
offload --dry-run          # against the real rig, per "Also before you leave"
```

**If the toolchain or Build Tools moved, `cargo clean` before that build.** Cargo
fingerprints `rustc`, not the linker, so a new MSVC toolset or CRT changes what the binary
is built from while cargo sees nothing to redo. Nothing relinks, and anything downstream
that never builds then re-checks bytes the *old* linker produced, and passes. A silent
false pass is the one outcome this step exists to prevent, so spend the half minute rather
than working out whether this particular change needed it.

**`cargo test` passing is not enough on a dependency bump.** The suite here is four tests
by design ([`DESIGN.md`](DESIGN.md) decision 18) and none of them touch a card reader, a
Thunderbolt hub or a real SSD. A crate that changed how a GPX timestamp parses or how a raw
header is read shows up in the dry run against the rig and nowhere else.

## Step 4 — commit the lockfile

`Cargo.lock` is committed in this repo, which is what makes an update a thing that happened
on a date rather than a thing that drifts. Commit it on its own, with the before-and-after
versions in the message, so a later bisect has something to aim at.

If output ever moves after a dependency update, **bisect rather than accept it**: revert
`Cargo.lock`, then re-apply one crate at a time with `cargo update -p <crate>`. A
dependency update changes no behavior this tool defines, so anything that moves is a
regression and you want to know which crate caused it.
