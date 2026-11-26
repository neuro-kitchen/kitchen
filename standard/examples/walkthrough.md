# Walking the examples through the spec

*Phase 1's "done when" ([roadmap](../roadmap.md)): each example walked step by step against the
spec, 2026-11-26, after decision 0015. ✓ the spec says it; ◐ partly; ✗ it does not (a question
below). Implementation work (things the spec allows but kitchen does not have yet) is marked
**build**.*

## 01. A plot is missing

| Step | Spec | |
|---|---|---|
| Claude reads types, steps, a real result over MCP | §6.1 `types`, `steps`, `results`, `result.read`; §6.2 MCP | ✓ (MCP last in 0.0.10) |
| `dspk:unit`, `events`, `figure` | §1.2 namespaced, §1.3 shared | ✓ |
| A Python step with `@dspk.op`, a numeric and a text parameter | §2.1 params, §2.2 script source | ✓ |
| `step.add` → Checked | §5.1 | ✓ |
| Preview of a checked script | §4.4, §5.3 | ✓ |
| Ana confirms, tied to the hash | §5.2 | ✓ |
| Listed where its inputs can be filled (curation's offers) | §4.5 offers; `steps` filters by one type | ◐ question W3 |
| The run stored by content ID, record with hash, params, inputs, `by`, `via` | §3.1, §3.2 | ✓ |
| The figure drawn by dsp-kitchen's viewer | decision 0013; core `interface/views` | **build** (the figure grammar and viewer) |
| "The lab's plugin folder, confirmed once by each person" | step discovery not in the spec; whose confirmation counts not said | ✗ questions W1, W2 |
| garnish restyles the same figure; lineage reaches the recording | §3.4 | ✓ |

## 02. Another interface

| Step | Spec | |
|---|---|---|
| Level A: workspaces as view-models composing existing ones | architecture (MVVM), decision 0010 | ✓ (the frame's arrangements exist) |
| `hello` over HTTP, `results` of type `table`, `labels` for **this slide** | §6.1 `results` filters by type, step, run, since | ◐ question W4 (filter by what a result came from) |
| `steps` finds the detection composite | §2.3, §6.1 | ✓ |
| `result.read` on the table | §6.1 | ✓ |
| lucem's viewer embedded in the page, tiles from the runtime | `result.read` by region (0015) | ✓ contract; **build** (viewers embeddable in a web page) |
| `run` + `run.watch` from the page | §6.1, §6.2 (SSE) | ✓ |
| `by` Ben `via` the page; may run, may not confirm | §6.3, §5.5 | ✓ |
| Results shared with every interface | §3.5 one store | ✓ |

## 03. A pipeline as a button

| Step | Spec | |
|---|---|---|
| The pipeline as data, with a confirmed script | §4.1 | ✓ |
| Saved as a step exposing two parameters | §2.3, §6.1 `pipeline.save_as_step` | ✓ |
| One confirmation covers it all (hash includes the script's) | §5.2 | ✓ |
| Pinned to Explore's offers | §4.5, §6.1 `offers` / `pin` | ✓ |
| Run on the current image, walked in tiles, `finish` stitches labels | §4.3, decision 0015 | ✓ |
| Shown by type: labels overlay, table panel | views by type (architecture) | ✓ contract; **build** (overlay and table views in the core) |
| A labmate adds the file to the lab's folder and confirms it | step folders, per-person confirmation | ✗ questions W1, W2 |

## 04. The hard cases

| Case | Spec | |
|---|---|---|
| A. Windows on the device, fitted whitening | §4.3, §3.3 (fit reused by run key), run options (0015) | ✓ |
| B. Tiles at a level, labels stitched | §4.3, `finish`, levels (0015) | ✓ |
| C. Previews while a slider moves | §4.4; streaming open in §6.4 | ◐ question O8 |
| D. Curation by people, two curators | §2.4, §3.4 | ✓ |
| E. Three apps, one lineage, run by the orchestrator | §3.4; §4.6 "pipelines spanning apps" open | ◐ question O7 |

## Questions to decide

From the walk (W) and the spec's own open questions (O). Each has a recommendation.

| # | Question | Recommendation |
|---|---|---|
| W1 | Whose confirmation counts: each person's, or the lab's? | Each person's by default (a confirmation is a record `by` someone, §5.4). A lab setting may trust confirmations by named people, so a step confirmed by the lab's maintainer runs for everyone; on the orchestrator, by role (§6.3). |
| W2 | Where does the runtime find steps (the lab's folder)? | Step folders, read in order: the project's, the person's, the lab's (a path or the orchestrator). A step found there is Unchecked until checked; nothing runs because of where it was found. |
| W3 | Listing the steps a screen could run | `steps` takes `fillable_by`: a screen's offers; it returns the steps whose required inputs can all be filled from them. |
| W4 | Results of one slide, one recording | `results` takes `from`: a source or result ID; it returns the results whose lineage reaches it. |
| O1 | Axes and units: part of the type, or of each result? (§1.6) | The type names its axes and their kind of unit (`[channels, samples]`, time); each result carries the values (sample rate, pixel size). |
| O2 | A small "is-a" list for types? (§1.4) | No, in this version. A step that accepts several types lists them; revisit when two apps need the same subtype. |
| O3 | Versions of built-in steps (§2.5) | A step may carry its own `version`, bumped by its domain when its behaviour changes; without one, the app's version. A domain's test of golden outputs per step catches a forgotten bump. |
| O4 | Steps with side effects (§2.5) | Allowed, marked `effects` in the description: never reused by run key, never previewed, and each run asks (as a confirmation) unless the person allowed it for the session. |
| O5 | Partial results of a cancelled run (§3.6) | Kept as results of a run with status `cancelled` and marked partial: visible, exportable, never reused by run key. |
| O6 | Fan-out: a "for each" node? (§4.6) | No. Batches (`run` with a list of fillings, 0015) and `many` ports cover it. |
| O7 | Pipelines spanning apps (§4.6) | A runtime hosts several domains and tools: the orchestrator hosts all of them and runs such a pipeline; an app's own runtime runs what its domain and the tools it has can run, and `pipeline.check` says which step it cannot. |
| O8 | Streamed previews (§6.4, hard case C) | One answer per call; a new `preview` from the same screen cancels the one still running. Simple over every transport; the slider feels live because previews are of one piece. |

When these are decided, the spec takes them in one pull request with a decision record, and
Phase 1 is done.
