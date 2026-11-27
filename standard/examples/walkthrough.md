# Walking the examples through the spec

*Phase 1's "done when" ([roadmap](../roadmap.md)): each example walked step by step against the
spec, 2026-11-26, after decision 0015. ✓ the spec says it; ◐ partly; ✗ it does not (a question
below). Implementation work (things the spec allows but kitchen does not have yet) is marked
**build**.*

## 01. A plot is missing

| Op | Spec | |
|---|---|---|
| Claude reads types, ops, a real result over MCP | §6.1 `types`, `ops`, `results`, `result.read`; §6.2 MCP | ✓ (MCP last in 0.0.10) |
| `dspk:unit`, `events`, `figure` | §1.2 namespaced, §1.3 shared | ✓ |
| A Python op with `@dspk.op`, a numeric and a text parameter | §2.1 params, §2.2 script source | ✓ |
| `op.add` → Checked | §5.1 | ✓ |
| Preview of a checked script | §4.4, §5.3 | ✓ |
| Ana confirms, tied to the hash | §5.2 | ✓ |
| Listed where its inputs can be filled (curation's offers) | §4.5 offers; `ops` filters by one type | ◐ question W3 |
| The run stored by content ID, record with hash, params, inputs, `by`, `via` | §3.1, §3.2 | ✓ |
| The figure drawn by dsp-kitchen's viewer | decision 0013; core `interface/views` | **build** (the figure grammar and viewer) |
| "The lab's plugin folder, confirmed once by each person" | op discovery not in the spec; whose confirmation counts not said | ✗ questions W1, W2 |
| garnish restyles the same figure; lineage reaches the recording | §3.4 | ✓ |

## 02. Another interface

| Op | Spec | |
|---|---|---|
| Level A: workspaces as view-models composing existing ones | architecture (MVVM), decision 0010 | ✓ (the frame's arrangements exist) |
| `hello` over HTTP, `results` of type `table`, `labels` for **this slide** | §6.1 `results` filters by type, op, run, since | ◐ question W4 (filter by what a result came from) |
| `ops` finds the detection composite | §2.3, §6.1 | ✓ |
| `result.read` on the table | §6.1 | ✓ |
| lucem's viewer embedded in the page, tiles from the runtime | `result.read` by region (0015) | ✓ contract; **build** (viewers embeddable in a web page) |
| `run` + `run.watch` from the page | §6.1, §6.2 (SSE) | ✓ |
| `by` Ben `via` the page; may run, may not confirm | §6.3, §5.5 | ✓ |
| Results shared with every interface | §3.5 one store | ✓ |

## 03. A pipeline as a button

| Op | Spec | |
|---|---|---|
| The pipeline as data, with a confirmed script | §4.1 | ✓ |
| Saved as an op exposing two parameters | §2.3, §6.1 `pipeline.save_as_recipe` | ✓ |
| One confirmation covers it all (hash includes the script's) | §5.2 | ✓ |
| Pinned to Explore's offers | §4.5, §6.1 `offers` / `pin` | ✓ |
| Run on the current image, walked in tiles, `finish` stitches labels | §4.3, decision 0015 | ✓ |
| Shown by type: labels overlay, table panel | views by type (architecture) | ✓ contract; **build** (overlay and table views in the core) |
| A labmate adds the file to the lab's folder and confirms it | op folders, per-person confirmation | ✗ questions W1, W2 |

## 04. The hard cases

| Case | Spec | |
|---|---|---|
| A. Windows on the device, fitted whitening | §4.3, §3.3 (fit reused by run key), run options (0015) | ✓ |
| B. Tiles at a level, labels stitched | §4.3, `finish`, levels (0015) | ✓ |
| C. Previews while a slider moves | §4.4, each app's own | ✓ (decision 0016) |
| D. Curation by people, two curators | §2.4, §3.4 | ✓ |
| E. Three apps, one lineage, run by the orchestrator | §3.4; §4.6 chains | ✓ (decision 0016) |

## Decided (decision 0016)

| # | Question | Decided |
|---|---|---|
| W1 | Whose confirmation counts | Confirmation moves to v0.2 (each person's, trusted maintainers, roles there); until then script ops run once checked |
| W2 | Where ops are found | The workspace next to the data (`.dspk/`, `.lucem/`: settings, keys, `ops/`, `recipes/`, `pipelines/`, `runs/`), then the person's configuration folder, then the lab's; plain files, exported and imported |
| W3 | Ops a screen can run | The runtime answers (`ops` with `fillable_by`); to be refined with use |
| W4 | What was made from this | Lineage read forward (`results` with `from`), only from written run records |
| O1 | Axes and units | The type names axes and unit kinds; values come from each result's metadata |
| O2 | Is-a | No inheritance in this version |
| O3 | Versions | Every op declares one, starting at 1; scripts and recipes use their hash |
| O4 | Side effects | `effects`: never reused, never previewed, asks before each run |
| O5 | Cancelled runs | Outputs kept and usable, marked *run incomplete*; a new run starts from the beginning |
| O6 | For each | No; batches and `many` ports |
| O7 | Across apps | Apps see only their ops; chains join apps, only on the orchestrator |
| O8 | Previews | Each app's own; the contract keeps only one piece, nothing stored, no record |

With these, Phase 1 is done: the examples walk through the spec, and what is left is building it.
