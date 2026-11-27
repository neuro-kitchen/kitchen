# 2. Ops

*Draft, from kitchen-pipeline's `OpSpec` and script interface, and kitchen-tool's `Operation`,
which this part merges.*

An **op** is one thing that turns inputs into outputs: a filter, a sorter, a cell detector, a
statistics test, a plot, a format conversion, a whole pipeline saved as a recipe.

The words of this standard: an **op** is one operation; a **pipeline** is a graph of ops inside one
app's workspace; a **recipe** is a pipeline saved as an op; a **chain** is the orchestrator's
sequence across apps ([§4.6](04-pipelines.md)).

## 2.1 What an op says about itself

| Field | Meaning |
|---|---|
| `id` | Unique within its namespace: `dspk:highpass`, `lucem:watershed`, `script:local_contrast` |
| `version` | Built-in and tool ops: a whole number every op declares, starting at 1, bumped by its app or tool when the op's behaviour changes (a test of golden outputs per op catches a forgotten bump). Scripts and recipes: their content hash |
| `title`, `family`, `summary` | For people: its name, its group in menus, one line |
| `inputs` | Ports: name, type(s), optional or not, one or many |
| `outputs` | Ports: name, type (at least one) |
| `params` | Name, kind (number with range and unit, whole number, switch, text, choice), default, a hint |
| `margin` | How far it looks beyond a piece, per axis: pixels around a tile, samples before and after a window |
| `source` | Where its code is: built in (app, version), a script (path, language, hash), a tool (name, operation, version), or a recipe (the pipeline's hash) |
| `finish` | Optional: how the outputs of its pieces are joined after the last one (lucem's labels stitched across tiles). Without it, outputs are joined in piece order ([§4.3](04-pipelines.md)) |
| `effects` | Set when the op acts outside the store (uploads to an archive, drives an instrument, sends a message, deletes a file): it is never reused from an earlier run, never previewed, and asks before each run unless the person allowed it for the session. Unset for nearly every op |
| `available` | Whether it can run here, and why not ("weights not downloaded", "needs CUDA"); a tool op that can be installed says so ([§6.1](06-runtime-api.md) `op.install`) |

An op's description is all a pipeline check, a menu, an inspector or an agent needs; nothing
about it requires running it.

## 2.2 Four sources, one description

| Source | Written as | Runs |
|---|---|---|
| **Built in** | The app's own code (Rust), registered by its domain | In the app's process, on its device |
| **Script** | A Python function decorated with `@op` (under the app's name: `@lucem.op`, `@dspk.op`), or a Rust or Python file with header lines | Out of process, by the runtime's workers, once checked ([§5](05-confirmation.md)) |
| **Tool** | A separate program answering describe / validate / run (neuro-convert, external sorters) | As a program, on whole results |
| **Recipe** | A pipeline saved as an op | By the engine, one op after another |

How scripts declare their op (decorators, signatures, header lines) is in the reference
implementation's documentation; the result is always this description.

### Where an app finds ops

An app lists its own built-in ops, the shared ones, its tools', and the script ops and recipes in
its folders. Another app's ops are ignored (dsp-kitchen does not list `lucem:watershed`); only the
orchestrator knows every app's. The folders, nearest first:

1. **The workspace**: a folder next to the data, named for the app, as `.vscode` is for an editor:

   ```text
   experiment-12/
   ├── session_12.bin
   └── .dspk/
       ├── settings.toml        local settings over the person's
       ├── keybindings.toml     local keys, if any
       ├── ops/                 script ops (.py, .rs)
       ├── recipes/             recipes (*.recipe.json)
       ├── pipelines/           pipelines being worked on
       └── runs/                run records (§3)
   ```
2. **The person's**: the app's own configuration folder, where its settings and keys already live
   (`~/.config/dspk-app/` on Linux and macOS, `%APPDATA%\dspk-app\` on Windows), with the same
   `ops/` and `recipes/`.
3. **The lab's**: through the orchestrator.

The nearest wins when two have the same id. Every op there is a plain file: sharing is a copy, and
the app's *Export ops and recipes…* bundles what a pipeline needs into one folder; importing puts
them in the workspace.

## 2.3 Recipes

A pipeline becomes an op when saved as one:

- its inputs are the pipeline's sources, its outputs the pipeline's outputs;
- its parameters are the ones the author chooses to expose (`threshold` from the node `detect`),
  the rest fixed;
- its version is the pipeline's content hash, which includes the hashes of its scripts;
- its margin is computed from the ops inside.

From outside it is an op like any other: it can be in a menu, a pipeline, or a button.

## 2.4 Manual ops

Some results are made by people: a curated unit, an accepted cell, a drawn region. These are
ops too, with `source` naming the screen and `by` the person in the run record ([§3](03-results.md)).
Curation decisions are then results with history, linked to what they judged.

## 2.5 Open questions

- None left from the first draft (versions and side effects: decision 0016).
