# kitchen-contract

The neuro-kitchen pipeline contract as Rust types: what every app, the runtime, a generated
interface and an agent agree on. It is the [standard](../../standard/README.md) in code.

| Module | Holds |
|---|---|
| `types` | `Type` (shared names and `app:name`), `Format`, matching, the shared types, old artifact kinds read as type and format |
| `params` | parameters: kinds, values, checking |
| `ops` | `OpSpec`: version from 1, ports, parameters, `Margin`, source (built in, script, tool, recipe), `finish`, `effects`, availability |
| `catalog` | an app's ops: by family, by input type, fillable by a screen's offers, by search |
| `pipelines` | `Pipeline` and its check: every problem at once, or the plan (order, feeds, parameters, margin) |
| `results` | `ContentId`, canonical JSON, `ResultRecord`, `RunRecord`, `RunKey` |

No I/O, nothing drawn, nothing of ours depended on. The JSON Schemas in
[`standard/schemas/`](../../standard/schemas/) are generated from these types; regenerate them with
`UPDATE_SCHEMAS=1 cargo test -p kitchen-contract`.
