# PhotoTravelWorkflow

One command, run from a hotel room at the end of a day's shooting, that lands four
verified copies of the day's photographs before bedtime.

Plug two card readers and three external SSDs into a Thunderbolt hub, run it, go to
dinner. It reads the cards, writes `YYYY/YYYY-MM-DD` directories to four destinations,
reads every byte back to prove the copies are bit-identical, geotags each frame from a
GPX track into a Lightroom-ready XMP sidecar, and prints a verdict you can read in five
seconds before putting an SSD in the safe.

All four copies are backups and none of them is a working copy — they are interchangeable
by construction, and editing happens at home from a NAS rather than from anything this
tool writes.

**Status: it works end to end, for the part that matters.** Pre-flight and phase 3 are
built, and have landed a real 3,883-frame, 201 GB shooting day onto four destinations in
20 minutes with every one of the 15,532 `(file, destination)` pairs read back off the
media and verified. Corroboration, geotagging, the report and eject are still to come.
The design is settled and written up in
[`docs/DESIGN.md`](docs/DESIGN.md) — numbered decisions, each with its reasoning,
covering the phase structure, why verification has to defeat both the OS page cache and
the SSD's own DRAM cache, how filenames stay deterministic across four destinations, and
where the wall-clock time actually goes.

Built for one specific rig — a Canon EOS R5 writing uncompressed CR3 to two cards
simultaneously, on Windows — and the design says so wherever that assumption is load
bearing.

## Documentation

| Document | Its reader |
|---|---|
| [`docs/CONOPS.md`](docs/CONOPS.md) | the operator, mid-trip — how this is intended to be used |
| [`docs/DESIGN.md`](docs/DESIGN.md) | someone changing the design |
| [`docs/REVIEWING.md`](docs/REVIEWING.md) | someone about to put a change on `main` |
| [`docs/UPDATING.md`](docs/UPDATING.md) | someone about to bump a dependency — or leave on a trip |
| [`docs/WRITING.md`](docs/WRITING.md) | someone writing or reviewing a document |

## License

MIT. See [`LICENSE`](LICENSE).
