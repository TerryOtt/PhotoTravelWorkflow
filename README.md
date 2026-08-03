# PhotoTravelWorkflow

One command, run from a hotel room at the end of a day's shooting, that lands four
verified copies of the day's photographs before bedtime.

Plug two card readers and three external SSDs into a Thunderbolt hub, run it, go to
dinner. It reads the day off the camera's cards, writes `YYYY/YYYY-MM-DD` directories to
four destinations, reads every byte back to prove the copies are bit-identical, geotags
each frame from a GPX track into a Lightroom-ready XMP sidecar, and prints a verdict you
can read in five seconds before putting an SSD in the safe.

**Status: designed, not yet implemented.** The design is settled and written up in
[`docs/DESIGN.md`](docs/DESIGN.md) — seventeen decisions with the reasoning behind each,
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
