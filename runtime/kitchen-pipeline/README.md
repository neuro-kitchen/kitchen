# kitchen-pipeline

The pipeline the neuro-kitchen apps share: a graph of operations as data, operations from the
app's Rust code and from Python and Rust scripts (read from their decorators, checked, then
confirmed by a person), and an engine that runs the graph piece by piece over whatever data the
app walks.

Every app takes Python and Rust code; how it goes through its data is its own. lucem walks a
slide in tiles with a margin of pixels; dsp-kitchen walks a recording in windows with samples
before and after. This crate is the part they share, and the walk is behind one trait,
[`Domain`](src/domain.rs).

```text
        the app (implements Domain)                  kitchen-pipeline
 ┌──────────────────────────────────┐   ┌──────────────────────────────────────────────┐
 │ port types: image, mask, signal… │   │ Graph: nodes, edges, params (JSON, versioned)│
 │ pieces: tiles / time windows     │◄──│ Catalog: operations from Rust, scripts, tools│
 │ read, trim, write one piece      │   │ check: types, ports, params, loops, context  │
 │ its own Rust operations          │──►│ scripts: decorators → ports and params;      │
 │ piece ⇄ exchange files (npy/csv) │   │          check → confirm → run               │
 └──────────────────────────────────┘   │ Engine: piece by piece, progress, cancel     │
                                        └──────────────────────────────────────────────┘
```

## Layout

```text
kitchen-pipeline/
├── python/
│   ├── kitchen_pipeline.py   the decorators (@op, @param, @input, @output), port types, describe()
│   ├── kitchen_runtime.py    the app's names for them (lucem, dspk), loading a script
│   ├── kitchen_probe.py      the check: import, describe the function
│   └── kitchen_worker.py     the warm worker: JSON lines, files in /dev/shm
├── src/
│   ├── types.rs              the contract's ports, params, margins (old names PortType, Context)
│   ├── op.rs                 Operation<D>, FnOp, Call (OpSpec, OpSource, Language: the contract's)
│   ├── catalog.rs            the contract's Catalog
│   ├── graph.rs              the contract's Pipeline and its check (old name Graph)
│   ├── domain.rs             Domain, Piece, Region, Exchange; windows() and tiles()
│   ├── engine.rs             Engine: run, preview, Progress, Cancel, RunReport
│   └── script/
│       ├── mod.rs            Script (read, hash, spec), scan()
│       ├── interface.rs      text → RawSignature → Interface (decorators and kitchen: headers)
│       ├── probe.rs          the check in its own process; mismatches with the text
│       ├── review.rs         Status, Approvals (approvals.json), ConfirmPolicy
│       ├── worker.rs         Tools, PythonWorker, RustBuild, ScriptOp
│       └── harness.rs        the std-only program a Rust script is compiled into
└── tests/engine.rs           tiles over a 2-D domain; a confirmed Python script as a node
```

No GUI and no GPU: serde, serde_json, thiserror and sha2 only.

## An operation in Python

```python
import numpy as np
import lucem            # dspk, or kitchen_pipeline: the same decorators

@lucem.op(family="Filter", halo=12)
@lucem.param("strength", min=0.0, max=5.0, hint="How much of the detail to add back")
def local_contrast_boost(image: lucem.Image, sigma: float = 3.0, strength: float = 1.8) -> lucem.Image:
    """Enhance fine cellular structures."""
    ...
```

- Arguments that are data become **inputs** (the type from the annotation, else the name, else
  the app's main type); `mask: Mask | None = None` is optional.
- Arguments with a default become **parameters**: `float` a number, `int` a whole number, `bool`
  a switch, `str` text, `Literal["a", "b"]` a choice. `@param` adds a range, a unit, a hint.
- The return annotation is the **output**; several outputs are declared with `@output` and
  returned as a dict.
- `halo=` (pixels around a tile) or `before=` / `after=` (samples around a window) is the
  **context** the engine reads around each piece.
- An argument named `tile` or `piece` receives where the piece sits.

## An operation in Rust

```rust
// kitchen: name = Band power
// kitchen: before = 300
// kitchen: input signal: signal
// kitchen: param low = 300.0 (1..15000) Hz
// kitchen: output power: signal
pub fn run(inputs: &Inputs, params: &Params) -> Result<Array, String> {
    let x = inputs.get("signal")?;
    let low = params.number("low")?;
    x.like(/* … */ x.data.clone())
}
```

Compiled once per version around the std-only harness (`Array`, `Inputs`, `Params`, `.npy` in and
out). The same header lines work in a Python file without decorators (`def run(...)`), and
kitchen-editor's `kitchen: node = in a:image; out b:mask` line is read too.

## Read, check, confirm, run

1. **Read**: `Script::read(path, &opts)` gives the interface, or the problems with it.
2. **Check**: `probe(&script, &tools, &opts)` imports it in its own process (or compiles it) and
   compares what the function declares with what the text says.
3. **Confirm**: `approvals.confirm(&script, Some(&probe), who)`; saved in the project's
   `approvals.json`. By default bound to the file's content: any edit asks again, saying whether
   inputs, outputs or parameters changed. `ConfirmPolicy::Interface` keeps it across edits that
   leave the interface alone. A preview may run a checked script; a run needs a confirmed one.
4. **Run**: `engine.add(ScriptOp::new(&script, Some(worker), None)?)`. The run's report records
   each script's hash, for provenance.

## What an app writes

| | lucem | dsp-kitchen |
|---|---|---|
| `Domain::Data` | an image block (GPU) | a window of channels (device) |
| `port_types` | image, mask, labels, table | signal, events, table |
| `pieces` | `tiles(shape, block, context)` | `windows(len, size, context)` |
| `read` / `trim` / `write` | Zarr levels; labels joined across tiles in `finish` | raw / Zarr; events appended |
| `exchange` | blocks ⇄ `.npy` (float32, uint32 labels) | windows ⇄ `.npy` `[channels, samples]` |
| Operations | `ops::Op` wrapped as `Operation` | dspk-ops stages wrapped as `Operation` |
| Script names | `Options::new("lucem", …).alias("lumen")` | `Options::new("dspk", …)` |

Each app keeps a reader for its existing spec files (lucem's `PipelineSpec`, dsp-kitchen's
`dspk_pipeline`) that turns them into a `Graph`.
