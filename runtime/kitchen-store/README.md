# kitchen-store

The store: what runs read and write, known by content, with the record of how each was made
([standard §3](../../standard/spec/03-results.md)).

On a person's machine it is the app's workspace next to the data:

```text
experiment-12/
├── session_12.bin
└── .dspk/
    ├── runs/       one JSON file per run (the contract's RunRecord), appended, never edited
    └── results/    one JSON file per result: its record and where it is (kept by reference)
```

- `Workspace::record_run`, `run`, `runs`: run records, written once.
- `Workspace::keep`, `result`, `locate`: results kept where they are; the id is checked before the
  place is given back, so a changed file is never taken for its result.
- `Workspace::lineage` (back to the sources) and `Workspace::made_from` ("what was made from
  this?"), built only on written records.
- `Workspace::export`: a result written where a person asks, its run record beside it.
- `identify`: the content id of a file or a Zarr folder.

A lab's orchestrator keeps the same records in SQLite. Was kitchen-artifacts until phase 2, step 3b;
its tool provenance and kinds stay until the tools move to the contract's types (step 3d).
