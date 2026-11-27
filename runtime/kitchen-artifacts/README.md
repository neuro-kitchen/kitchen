# kitchen-artifacts

What the neuro-kitchen tools read and write, as the orchestrator (`kitchen-orchestrate`) sees it.
Every step of a chain reads and writes **artifacts**:

```
raw (SpikeGLX, CZI…) → NWB-Zarr / OME-Zarr → sorting / labels (Zarr) → Parquet / GeoParquet
```

The orchestrator connects tools only by their artifacts' kinds, so a new tool joins without any
change to it.

**Status: written, not built yet.**

| Piece | What |
|---|---|
| `ArtifactKind` | `family/format`: `raw/*`, `zarr/*` (stores), `table/*`, `file/*`; constants for today's kinds |
| `KindPattern` | what a tool accepts: one kind, or a whole family (`raw/*`) |
| `identify`, `ArtifactId` | content identity (see below) |
| `Provenance` | the artifact, the tool (version, image by digest), operation, recipe, inputs, parameters, run key, times |
| `RunKey` | the hash of everything that decides a run's result: same key, same result, reused |

## Identity

- **A file:** the SHA-256 of its bytes, the same as `sha256sum`.
- **A store:** the SHA-256 of a manifest listing every file's path, size and SHA-256, sorted by
  path.
  - Chunks are hashed by content: equal-size float chunks almost always differ, so size alone
    would merge different data.
  - The store's provenance file is left out, so writing it never changes the id it records.
  - Symbolic links inside a store are refused.

Hashing reads every byte. The orchestrator caches ids by path, size and modification time.

## Provenance on disk

The record travels with the data, so it survives copies, archives and sharing:
- **A store:** `kitchen-provenance.json` at its root. Zarr readers ignore unknown files.
- **A file:** `<file>.provenance.json` beside it.

Embedding the record in Parquet's own key-value metadata can come later; the sidecar works for
every kind.
