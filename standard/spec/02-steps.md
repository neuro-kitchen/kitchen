# 2. Steps

*Draft, from kitchen-pipeline's `OpSpec` and script interface, and kitchen-tool's `Operation`,
which this part merges.*

A **step** is one thing that turns inputs into outputs: a filter, a sorter, a cell detector, a
statistics test, a plot, a format conversion, a whole pipeline.

## 2.1 What a step says about itself

| Field | Meaning |
|---|---|
| `id` | Unique within its namespace: `dspk:highpass`, `lucem:watershed`, `script:local_contrast` |
| `version` | Built-in and tool steps: the app's or tool's version. Scripts and composites: their content hash |
| `title`, `family`, `summary` | For people: its name, its group in menus, one line |
| `inputs` | Ports: name, type(s), optional or not, one or many |
| `outputs` | Ports: name, type (at least one) |
| `params` | Name, kind (number with range and unit, whole number, switch, text, choice), default, a hint |
| `margin` | How far it looks beyond a piece, per axis: pixels around a tile, samples before and after a window |
| `source` | Where its code is: built in (app, version), a script (path, language, hash), a tool (name, operation, version), or a composite (the pipeline's hash) |

A step's description is all a pipeline check, a menu, an inspector or an agent needs; nothing
about it requires running it.

## 2.2 Four sources, one description

| Source | Written as | Runs |
|---|---|---|
| **Built in** | The app's own code (Rust), registered by its domain | In the app's process, on its device |
| **Script** | A Python function decorated with `@op` (under the app's name: `@lucem.op`, `@dspk.op`), or a Rust or Python file with header lines | Out of process, by the runtime's workers; only once confirmed |
| **Tool** | A separate program answering describe / validate / run (neuro-convert, external sorters) | As a program, on whole results |
| **Composite** | A pipeline saved as a step | By the engine, step by step |

How scripts declare their step (decorators, signatures, header lines) is in the reference
implementation's documentation; the result is always this description.

## 2.3 Composite steps

A pipeline becomes a step when saved as one:

- its inputs are the pipeline's sources, its outputs the pipeline's outputs;
- its parameters are the ones the author chooses to expose (`threshold` from the node `detect`),
  the rest fixed;
- its version is the pipeline's content hash, which includes the hashes of its scripts;
- its margin is computed from the steps inside.

From outside it is a step like any other: it can be in a menu, a pipeline, or a button.

## 2.4 Manual steps

Some results are made by people: a curated unit, an accepted cell, a drawn region. These are
steps too, with `source` naming the screen and `by` the person in the run record ([§3](03-results.md)).
Curation decisions are then results with history, linked to what they judged.

## 2.5 Open questions

- Versions of built-in steps: the app's version is coarse (every release changes every step's
  version, so nothing is reused across releases). A per-step version the app bumps when the step's
  behaviour changes is finer, and relies on discipline.
- Steps with side effects (writing to an instrument, uploading to an archive): allowed, and marked,
  so they are never reused from a previous run?
