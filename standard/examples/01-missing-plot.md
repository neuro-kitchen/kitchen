# 01. A plot is missing; Claude makes a reusable one

*Ana is in dsp-kitchen's Analysis screen and wants a plot of a unit's firing rate across the
session, split by trial condition.*

## Walk-through

1. **Ana asks Claude** (in dsp-kitchen's chat panel, or Claude Code beside it): "Add a plot of the
   firing rate over time per condition for the selected unit."
2. **Claude reads the runtime** over MCP: `types` (`dspk:unit`, `events`, `table`, `figure`),
   `ops` that produce them, and one real result (`results` + `result.read`) to see its shape.
3. **Claude writes an op**, a Python file in the workspace (`.dspk/ops/firing_rate_by_condition.py`,
   next to the recording, [spec §2.2](../spec/02-ops.md)):
   ```python
   import dspk

   @dspk.op(family="Plots")
   @dspk.param("bin_ms", 500, min=10, max=10_000, unit="ms")
   def firing_rate_by_condition(unit: dspk.Unit, trials: dspk.Events,
                                bin_ms: int = 500, condition: str = "condition") -> dspk.Figure:
       """Firing rate of a unit over the session, one line per trial condition."""
       ...
   ```
   It works on any unit and any trials, not only Ana's: the op is the reusable part.
4. **`op.add`**: the runtime reads its description and checks it (loads in a separate process,
   declares what its text says). State: Checked.
5. **Preview**: dsp-kitchen shows the figure for the selected unit at once (`preview`).
6. **It appears where it fits**: Analysis offers *selected unit* (`dspk:unit`) and *trials*
   (`events`); `ops` with `fillable_by` those offers lists it in Analysis's Plots menu (and not
   `kilosort`, which needs a whole recording), and in the Pipeline's Operations.
7. **Ana runs it** (it is checked, so it runs): the `figure` is stored by content ID; the run record,
   in `.dspk/runs/`, names the script's hash, `bin_ms`, the unit and trials by ID, `by` Ana, `via`
   dsp-kitchen. dsp-kitchen's figure viewer shows
   it.
8. **Reuse**: next session, the same op on another unit; for herself everywhere, the file copied to her own
   `~/.config/dspk-app/ops/`; for the lab, the lab's folder through the orchestrator; for a paper, garnish restyles the same `figure`, whose
   lineage reaches the raw recording.

## Requires

- The `figure` type, with a static format and an interactive one ([spec §1](../spec/01-types.md)).
- A figure viewer every app uses (core `interface/views`).
- Offers: a screen's view-model states what it offers by type ([spec §4.5](../spec/04-pipelines.md)).
- Script ops found in the workspace, checked, previewed ([spec §2.2](../spec/02-ops.md), [§5](../spec/05-confirmation.md));
  confirmation joins in v0.2.
