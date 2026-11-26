# 4. Pipelines

*Draft, from kitchen-pipeline's graph and engine.*

## 4.1 A pipeline is data

```json
{
  "version": 1,
  "sources": {"recording": "signal"},
  "nodes": {
    "hp":   {"step": "dspk:highpass", "params": {"cutoff_hz": 300}},
    "sort": {"step": "dspk:kilosort", "params": {}}
  },
  "edges": [["recording", "hp.signal"], ["hp.signal", "sort.signal"]],
  "outputs": {"units": "sort.sorting"}
}
```

- **Sources**: the pipeline's inputs, by name, with their type. What fills them (which recording,
  the current image, at which pyramid level) is decided when it runs; a domain may fill one source
  from several files read as one (dsp-kitchen's merge).
- **Nodes**: a step and its parameters; optionally disabled (it passes an input of the same type on).
- **Edges**: `node.port` or a source's name, to `node.port`.
- **Outputs**: what the run keeps, by name.

A pipeline's content hash is its version. What it holds is processing only: how a result is drawn
(a view, its colours) belongs to the screen's view-model, which names a pipeline output; how it
runs (device, piece size, threads) is an option of each run ([§6.1](06-runtime-api.md)), so the
same pipeline runs anywhere.

## 4.2 Checking

Before anything runs, every problem is reported at once:

- unknown steps; parameters out of range or unknown (defaults filled for the rest);
- an output's type that does not match the input it feeds;
- a required input left unconnected, or fed twice;
- a loop;
- a disabled node that cannot pass anything of its outputs' types on;
- scripts and composites that are not confirmed ([§5](05-confirmation.md)).

A checked pipeline gives the order to run (only what leads to an output), and the **margin** its
sources must be read with: margins add up along a chain, the larger wins where branches meet.

## 4.3 Pieces: how data is walked

The contract does not say how data is cut; each app's domain does. What the contract fixes is the
shape every walk has:

- a **piece** owns a part of the data (a window of time, a tile) and is read with the margin
  around it, clipped to the data;
- every node runs on that read part; outputs are trimmed back to what the piece owns before they
  are written;
- the result over all pieces equals the result over the whole, for every step that looks no further
  than its margin.

- after the last piece, each step's outputs are joined: in piece order, or by the step's
  `finish` ([§2.1](02-steps.md)) when pieces must be joined across their borders (labels of cells
  cut by tiles).

An image may be walked at one level of its pyramid: the level is part of how the source is filled,
so the run record says which level was read.

dsp-kitchen walks windows; lucem walks tiles; garnish has one piece (the whole table); a tool works
on whole results. [examples/04-hard-cases.md](../examples/04-hard-cases.md) is required reading.

## 4.4 Preview

A preview is a run over one piece (the part on screen, possibly at a coarser pyramid level) whose
outputs are returned instead of stored. It is how a slider dragged in an inspector updates the view. A preview may use checked but
unconfirmed scripts ([§5](05-confirmation.md)); it writes no run record.

## 4.5 Context inputs

A screen offers what it currently shows as typed inputs: lucem's Explore offers *current image*
(`image`) and *selected region* (`region`); dsp-kitchen's curation offers *current unit*
(`dspk:unit`) and *current recording* (`signal`). A step or pipeline whose inputs can all be filled
from what a screen offers can be **pinned** there as a command (a button, a menu item, a key): the
screen fills the inputs when it is pressed.

The contract defines the offer (name, type) and the pin (step, which offer fills which input);
how a screen shows its pinned commands is the app's.

## 4.6 Open questions

- Fan-out: a step run once per item of a list (each recording of a session). A port marked `many`
  covers inputs; does the graph need a "for each" node too?
- Live sources (a device streaming from a sensor, dsp-kitchen's taps): not in this version. A live
  chain is still checked as a pipeline; running one is a later capability (`live`, §7.2).
- Pipelines spanning apps (convert in neuro-convert, sort in dsp-kitchen, statistics in garnish):
  the orchestrator runs them; does a single app need to?
