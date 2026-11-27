# kitchen-tools

Tools and plugins for the neuro-kitchen apps: separate programs an app or the orchestrator runs,
never links. `plugin` is the protocol (JSON lines over stdin and stdout, a handshake, progress),
discovery, a client and the manager (its Settings page is kitchen-shell's `plugins_view`). `tool`
is how the orchestrator (`kitchen-orchestrate`) drives a tool without its window: a tool is a
plugin program that answers three more requests. Was kitchen-plugin and kitchen-tool until phase 2,
step 3d; the tool protocol is version 2 since (ports as the contract's type and format).

| Request | Answer |
|---|---|
| `describe` | its operations, what each reads and writes (the contract's type and format), its recipe's JSON Schema, its resources |
| `validate` | whether a recipe can run, and if not, why |
| `run` | runs an operation on input results into an empty output folder; the outputs, review flags, a summary |

The orchestrator knows nothing else about a tool: a new tool joins the soup by answering these
three requests.

**Status: written, not built yet.**

## A tool's side

Implement `HeadlessTool` and answer from a `tool serve` subcommand (`dspk serve` and `lucem serve`
are already the network servers):

```rust
match args.command {
    Command::Tool(ToolCommand::Serve) => kitchen_tool::serve_stdio(&DspkTool)?,
    // …
}
```

The serve loop:
- refuses to run a recipe that doesn't validate;
- checks every report against the operation's declared outputs (port, kind, inside the output
  folder).

## The orchestrator's side

```rust
let mut tool = ToolClient::start("podman", &["run", "--rm", "-i", image, "dspk", "tool", "serve"])?;
let report = tool.run(&request, &mut |stage, fraction| { /* progress */ })?;
```

- Inputs are checked against the operation's ports before anything is sent.
- The run's key (`RunRequest::run_key`) is what the orchestrator uses to reuse a finished run.
- Provenance records (`kitchen-store`) are written by the orchestrator, which knows the image
  and the times. Tools only report what they wrote.

## The first tools

| Tool | Operation (to add) | Reads | Writes |
|---|---|---|---|
| neuro-convert | `convert` | `raw/*` | `zarr/nwb` |
| dspk | `sort` | `zarr/nwb` | `zarr/sorting`, `table/parquet` |
| lucem | `segment` | `zarr/ome` | `zarr/labels`, `table/geoparquet` |
