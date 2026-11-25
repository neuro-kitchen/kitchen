# 3. Results

*Draft, from kitchen-artifacts (identity, provenance, run keys) and kitchen-pipeline's run report,
which this part merges.*

## 3.1 A result

Each output a run produces is a **result**:

| Field | Meaning |
|---|---|
| `id` | Its content ID: `sha256:` and 64 hex digits, the SHA-256 of its bytes (of a canonical manifest for folders such as Zarr stores) |
| `type`, `format` | What it is and how it is encoded ([§1](01-types.md)) |
| `size` | Bytes |
| `run` | The run that produced it |

A result never changes; a different content is a different result. The same content produced twice
is the same result.

## 3.2 A run record

Every run of a step (or a pipeline) leaves one record:

| Field | Meaning |
|---|---|
| `run` | Its ID |
| `step` | The step's `id` and `version` (or hash) |
| `params` | Every parameter, defaults filled |
| `inputs` | Port → result IDs (or, for a source the app reads, the source's description and its ID) |
| `outputs` | Port → result IDs |
| `by` | Who: a person |
| `via` | Through what: an app, a generated interface, an agent |
| `started`, `ended`, `status` | When; `succeeded`, `failed` (with why), `cancelled` |
| `environment` | The runtime's version, the device, the app's version |
| `run_key` | See §3.3 |

Records are appended and never edited. A correction is a new run.

## 3.3 Run keys and reuse

`run_key` is a hash of: the step's id and version, the parameters, the input result IDs. Two runs
with the same key would produce the same results, so the runtime reuses the earlier results instead
of running again (and records that it did). Steps marked as having side effects, or as not
deterministic, are never reused.

## 3.4 Lineage

Run records link results to the runs that made them, and runs to their inputs. Walking back from
any result reaches the raw data: "figure 3 came from *plot by condition* (script, hash …) on the
sorting of session 12 (*kilosort* 4.0.1, these parameters), on recording `sha256:…`". The same walk,
exported, is a W3C PROV document or an RO-Crate, for a paper or an archive.

## 3.5 Where results live

In the runtime's **store**: on a laptop (a folder and SQLite), on a lab server (the orchestrator's
database and object storage). The store keeps results by content ID and run records by run. Moving
between stores is an export (results + records) and an import; IDs do not change.

## 3.6 Open questions

- Results as large as a multi-hour recording: kept by reference (the file where it is, with its ID
  checked) rather than copied into the store?
- Partial results of a cancelled run: kept (useful, and expensive to recompute) or discarded?
