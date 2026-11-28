# Decisions

One page per decision: what was decided, why, and what was given up. Append-only: a decision is
changed by a new one that replaces it, never by editing the old. When someone asks "why don't we
just add X?", the answer is usually here.

| # | Decision | State |
|---|---|---|
| [0001](0001-narrow-waist.md) | Standardize the narrow waist, and nothing above | Principle kept; scope replaced by 0008 |
| [0002](0002-append-only-whole-bodies.md) | History is append-only; a change carries the whole body | Accepted, applies to run records and decisions |
| [0003](0003-json-jcs-sha256.md) | JSON, canonicalized by RFC 8785, named by SHA-256 | Accepted |
| [0004](0004-json-schema-not-rdf.md) | Schemas are JSON Schema, not RDF or a new language | Accepted |
| [0005](0005-agents-are-actors.md) | Agents are actors with grants; actions that matter wait for approval | Accepted |
| [0006](0006-schema-ids-on-domains.md) | Schema IDs live on their publisher's domain | Replaced by 0012 |
| [0007](0007-working-name.md) | "Strata" is a working name until publication | Accepted |
| [0008](0008-the-contract-is-the-pipeline.md) | The standard is the pipeline contract | Accepted |
| [0009](0009-one-shared-runtime.md) | One shared runtime, not a back end per app | Accepted |
| [0010](0010-apps-are-mvvm.md) | Apps are MVVM; views never call the runtime | Accepted |
| [0011](0011-migration-order.md) | Young projects first: garnish and the orchestrator, neuro-convert, then dsp-kitchen and lucem | Accepted |
| [0012](0012-namespaced-types.md) | Types: shared names, or namespaced app names | Accepted |
| [0013](0013-figures-are-a-grammar-in-rust.md) | Figures are our own grammar of graphics (after ggplot2), drawn in Rust | Accepted |
| [0014](0014-the-standard-lives-in-kitchen.md) | The standard lives in kitchen, beside its implementation | Accepted |
| [0015](0015-what-lucem-and-dsp-kitchen-need.md) | Ten additions from reading lucem and dsp-kitchen: batch runs, export, finish, run options, regions, availability, metrics, levels; no live runs yet; display out of pipelines | Accepted |
| [0016](0016-ops-workspaces-and-checking-first.md) | Ops, pipelines, recipes, chains; workspaces next to the data; checking now, confirmation in v0.2; and the twelve questions of the walkthrough | Accepted |
| [0018](0018-compute-below-everything.md) | Compute is the bottom layer and its own repository (CubeCL pinned and re-exported); "backend" for CUDA/wgpu/CPU, "runtime" for kitchen's back end; libraries apart from apps, never depending on them | Accepted |
| [0017](0017-the-runtime-api-on-the-wire.md) | The runtime API in protobuf, over WebTransport (HTTP/3) with Connect as the TCP fallback; JSON documents inside, Arrow tables, raw array chunks; TOML on disk for every app | Accepted |

Format: **Context** (what forced a choice), **Decision**, **Consequences** (good and bad),
**Alternatives** considered.
