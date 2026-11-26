# 01. A plot is missing; Claude makes a reusable one

*Ana is curating units in dsp-kitchen and wants a plot of a unit's firing rate across the session,
split by trial condition.*

## Walk-through

1. **Ana asks Claude** (in dsp-kitchen's chat panel, or Claude Code beside it): "Add a plot of the
   firing rate over time per condition for the selected unit."
2. **Claude reads the runtime** over MCP: `types` (`dspk:unit`, `events`, `table`, `figure`),
   `steps` that produce them, and one real result (`results` + `result.read`) to see its shape.
3. **Claude writes a step**, a Python file:
   ```python
   import dspk

   @dspk.op(family="Plots")
   @dspk.param("bin_ms", 500, min=10, max=10_000, unit="ms")
   def firing_rate_by_condition(unit: dspk.Unit, trials: dspk.Events,
                                bin_ms: int = 500, condition: str = "condition") -> dspk.Figure:
       """Firing rate of a unit over the session, one line per trial condition."""
       ...
   ```
   It works on any unit and any trials, not only Ana's: the step is the reusable part.
4. **`step.add`**: the runtime reads its description and checks it (loads in a separate process,
   declares what its text says). State: Checked.
5. **Preview**: dsp-kitchen shows the figure for the selected unit at once (`preview`; checked
   scripts may preview).
6. **Ana confirms it** (`step.confirm`, tied to the file's hash). State: Confirmed.
7. **It appears where it fits**: curation's view-model offers *current unit* (`dspk:unit`) and
   *trials* (`events`); the step's inputs can be filled from those, so it is listed in curation's
   plots and in the Pipeline's Operations.
8. **Ana runs it**: the `figure` is stored by content ID; the run record names the script's hash,
   `bin_ms`, the unit and trials by ID, `by` Ana, `via` dsp-kitchen. dsp-kitchen's figure viewer shows
   it.
9. **Reuse**: next session, the same step on another unit; for the lab, the file in the lab's plugin
   folder, confirmed once by each person; for a paper, garnish restyles the same `figure`, whose
   lineage reaches the raw recording.

## Requires

- The `figure` type, with a static format and an interactive one ([spec §1](../spec/01-types.md)).
- A figure viewer every app uses (core `interface/views`).
- Offers: a screen's view-model states what it offers by type ([spec §4.5](../spec/04-pipelines.md)).
- Script steps checked, previewed, confirmed ([spec §5](../spec/05-confirmation.md)).
