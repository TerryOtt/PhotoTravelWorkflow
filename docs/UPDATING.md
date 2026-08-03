# Updating dependencies

**Cadence: once per photo trip, before you leave.** Not on the road. The whole point of
doing it on a schedule is that a surprise — a crate that changed behavior, a toolchain that
needs reinstalling, a config that no longer matches the hardware — surfaces while you are
at home with the rig and a real network connection, rather than in a hotel room with two
card readers and 2,000 photographs waiting to be offloaded.

If a trip is imminent and the update looks at all interesting, **skip it and travel with
the binary you have.** A version that is four point releases behind and verified is worth
more than a current one you have not run against real cards.

## Also before you leave

**Run a full dry run against the actual rig.** Plug in both readers and all three SSDs,
and:

```
photoday --dry-run
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
forever. The exposure concentrates in the pre-1.0 crates, and this project's set will be
GPS, time and raw-metadata handling, which is exactly where it lands. List them here once
`Cargo.toml` exists rather than leaving this paragraph abstract.

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
photoday --dry-run          # against the real rig, per "Also before you leave"
```

**If the toolchain or Build Tools moved, `cargo clean` before that build.** Cargo
fingerprints `rustc`, not the linker, so a new MSVC toolset or CRT changes what the binary
is built from while cargo sees nothing to redo. Nothing relinks, and anything downstream
that never builds then re-checks bytes the *old* linker produced, and passes. A silent
false pass is the one outcome this step exists to prevent, so spend the half minute rather
than working out whether this particular change needed it.

**`cargo test` passing is not enough on a dependency bump.** The suite here is three tests
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
