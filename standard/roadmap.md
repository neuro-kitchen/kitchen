# Roadmap

The order of [decision 0011](decisions/0011-migration-order.md): build what exists, write the
standard, restructure the core, then move projects from the youngest to the largest. Each phase
ships on its own and ends with a "done when".

## Phase 0: build what exists (done: kitchen 0.0.9)

The kitchen branches (`integration/0.0.9`, `feat/design-language`, `feat/pipeline`) hold the design
language, the frame, the file picker, the views, the code editor and kitchen-pipeline, none of it
built yet (when this phase was written). Nothing is moved or split before it compiles: moving unbuilt code makes every error
ambiguous.

- Build and test in kitchen's staged order (`scripts/check-all.sh`), fix, merge, tag 0.0.9.
- **Done when:** the branches are merged into kitchen's main and its tests pass. **Done** on
  2026-11-26: built and tested in the cloud (stages A–C, now in CI on every pull request), the
  libraries upgraded to their latest (gpui-kit 0.11), released as 0.0.9: the last kitchen from
  before the standard.

## Phase 1: the standard (kitchen's `standard/`; this week)

- The contract (types, steps, results, pipelines, confirmation), the runtime API, versioning,
  architecture, the examples. Drafts written from kitchen's code; corrected as phases 2–3 build it.
- Moved into kitchen ([0014](decisions/0014-the-standard-lives-in-kitchen.md)), so the contract
  and its code change in one pull request.
- **Done when:** each example (01–04) can be walked through on paper against the spec, and what
  lucem and dsp-kitchen's back ends do has a place in it (their gap lists).

## Phase 2: the core restructured (kitchen 0.0.10; next week)

In this order, each a pull request that builds and passes the tests:

1. **Move only**: `contract/`, `runtime/`, `interface/` folders; crates moved with history and
   renamed; the layer check in CI. No behaviour changes.
2. **Extract the back end** from interface crates: script running out of the editor; jobs, activity,
   environments and file changes out of the frame; the MCP host out of the chat panel; the pipeline
   engine into the runtime.
3. **Merge the contract**: one type system (port types and artifact kinds), one step description,
   one run record, one pipeline format, one version.
4. **The runtime API**: the client (in-process), the store, the server (HTTP, stdio); MCP joins
   the same server once the rest is in.
- **Done when:** the core's interface crates hold no back-end logic, and a script, a test and an agent
  reach the runtime through the same calls. Released as 0.0.10; kitchen's releases stay on
  `0.0.x`.

## Phase 3: garnish and the orchestrator

- **garnish**: reorganized into `domain/` (model, operations, formats) and `app/` (view-models,
  views) on the core's client; its own server, command line, MCP and pipeline format removed.
- **The orchestrator**: the core's runtime deployed for a lab: queue, shared database, dashboard.
- **Done when:** example 01 works in garnish (a plot added as a confirmed step and reused), and
  example 02 works against the orchestrator (a generated interface reaches the same runtime as
  garnish's window).

## Phase 4: neuro-convert

- Its operations described as steps, its formats as types, its outputs recorded.
- **Done when:** convert → garnish statistics runs in the orchestrator, with lineage from the figure
  to the vendor file.

## Phase 5: dsp-kitchen

- In this order: script running, run records, the server, the pipeline format (its spec read and
  converted), its domain (windows, device steps) on the core's engine; then screens as view-models.
- **Done when:** example 04 A and D pass, and dsp-kitchen carries no server, command line, MCP host,
  script runner or pipeline format of its own.

## Phase 6: lucem

- The same order; its tiles and stitching on the core's engine.
- **Done when:** example 03 works (a pipeline as an Explore button) and example 04 B, C and E pass.
  The contract can then be called 1.0.

## Phase 7: the template

- kitchen-template generates only a domain, view-models and views, and a Python package.
- Can move earlier: **before any new project is generated from it**.
- Started: template 0.0.3 has `domain/` and the app as view-models and views. Still its own:
  the pipeline, the session document, the server, the command line and MCP, replaced by the
  core's runtime once phase 2 has it.

## Before publication

- Choose the public name ([0007](decisions/0007-working-name.md)); check registries, domains,
  trademarks; rename.
- The conformance suite, run against the core.
- Split the standard out of kitchen again under its public name, make it public and invite
  comments on the contract ([0014](decisions/0014-the-standard-lives-in-kitchen.md)).
