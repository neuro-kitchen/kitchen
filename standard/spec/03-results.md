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

Every run of an op (or a pipeline) leaves one record:

| Field | Meaning |
|---|---|
| `run` | Its ID |
| `op` | The op's `id` and `version` (or hash) |
| `params` | Every parameter, defaults filled |
| `inputs` | Port → result IDs (or, for a source the app reads, the source's description and its ID) |
| `outputs` | Port → result IDs |
| `by` | Who: a person |
| `via` | Through what: an app, a generated interface, an agent |
| `started`, `ended`, `status` | When; `succeeded`, `failed` (with why), `cancelled` (its outputs so far are kept, marked incomplete: §3.3) |
| `environment` | The runtime's version, the device, the app's version, and the run's options (piece size, read-ahead, threads, upload mode) |
| `metrics` | Optional: time per op, bytes read, moved and written. For people tuning a run; never part of the run key |
| `run_key` | See §3.3 |

Records are appended and never edited. A correction is a new run.

## 3.3 Run keys and reuse

`run_key` is a hash of: the op's id and version, the parameters, the input result IDs. Run
options and metrics are not in it: they change how fast, not what. Two runs
with the same key would produce the same results, so the runtime reuses the earlier results instead
of running again (and records that it did). Never reused:

- ops marked `effects` ([§2.1](02-ops.md)): an upload done on Monday is not "already done" for a
  file fixed on Tuesday;
- ops marked as not deterministic;
- a cancelled run's outputs: they can be read and exported, their metadata says *run incomplete*,
  and running again starts from the beginning.

## 3.4 Lineage

Run records link results to the runs that made them, and runs to their inputs. Walking back from
any result reaches the raw data: "figure 3 came from *plot by condition* (script, hash …) on the
sorting of session 12 (*kilosort* 4.0.1, these parameters), on recording `sha256:…`". The same walk,
exported, is a W3C PROV document or an RO-Crate, for a paper or an archive.

The same records read forward answer **"what was made from this?"**: open a recording and see the
filtered copy, the sorting, the figures and the tables made from it, in any app, by anyone
(`results` with `from`, [§6.1](06-runtime-api.md)). Nothing is shown that is not in a written
record.

## 3.5 Where results live

In the runtime's **store**: on a laptop, the workspace next to the data (its run records in
`.dspk/runs/` or `.lucem/runs/`, [§2.2](02-ops.md)), so the history travels with the folder; on a
lab server, the orchestrator's database and object storage. The store keeps results by content ID and run records by run. Moving
between stores is an export (results + records) and an import; IDs do not change.

- **Writing to a path is an export.** A run's outputs always go to the store; writing one where
  a person asks (`{stem}.zarr` next to the recording) is an export of that result, with its record
  beside it.
- **Huge results are kept by reference**: the file stays where it is, the store keeps its ID and
  where it is, and checks the ID when the result is read. Exporting such a result is not a copy.

## 3.6 Open questions

- None left from the first draft (cancelled runs: decision 0016).
