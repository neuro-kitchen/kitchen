# Architecture

How neuro-kitchen is organized around the contract: four layers, MVVM in every app, and the
repository trees each project moves to.

## The layers

| Layer | Name | Holds | Depends on |
|---|---|---|---|
| The waist | **contract** | Types, ops, results, pipelines, confirmation. No I/O, no interface | nothing of ours |
| Shared back end | **runtime** | Runs ops and pipelines piece by piece, scripts, compute; stores results and run records; serves the runtime API | contract |
| Front of house | **interface** | Theme, widgets, frame, views, editor, chat panel | the runtime **only through its client** |
| An app's own part | **domain** | Its types, operations, file formats, how it walks its data | contract (and runtime's compute) |

Dependencies point down only; a check in CI fails otherwise.

## Apps are MVVM

| MVVM | In an app | Built on |
|---|---|---|
| **Model** | `domain/` (types, operations, formats) and the runtime, reached through `client` | contract, runtime |
| **ViewModel** | `app/viewmodels/`: one per screen; its state, its commands, and the inputs it offers (Explore offers *current image*, *selected region*) | the frame's command registry and workspaces; `client` |
| **View** | `app/views/`: GPUI drawing only | interface (theme, widgets, frame, views) |

**Rule:** views never call the runtime; only view-models do. Screens are therefore testable
headless, and what a screen offers (its inputs, its commands) is the same thing a generated
interface or an agent reaches through the runtime API.

## Repository trees

Folder and crate names say what they do. The project prefix of published packages (today
`kitchen-`) is decided once, with the public name.

### The core (today: `kitchen`)

```text
core/
├── contract/                  ← the waist: depends on nothing of ours
│   └── contract               types · ops · results · pipelines · recipes ·
│                              context inputs · confirmation
├── runtime/                   ← one back end for every app
│   ├── engine                 runs pipelines piece by piece; hosts each app's domain
│   ├── scripts                read · check · warm workers (Python, Rust); confirm in v0.2
│   ├── store                  results by content id · run records · run keys
│   ├── server                 the runtime API over HTTP · MCP · stdio
│   ├── client                 the same API in-process or remote
│   ├── services               jobs · activity · environments · file changes
│   ├── compute                GPU / CPU runtimes
│   ├── network                certificates, pairing, encrypted transport
│   ├── downloads              verified downloads, model cache
│   └── tools                  external tool programs over JSON lines
├── interface/                 ← talks to the runtime only through client
│   ├── theme                  design tokens, palettes, fonts, icons
│   ├── widgets                buttons, rows, inputs, parameter editor
│   ├── frame                  title bar, menus, workspaces, status bar, settings, file picker
│   ├── views                  dock; viewers by result type (figure, table, video…)
│   ├── editor                 code editor, change marks
│   └── chat                   the agent chat panel
├── python/                    decorators (@op …) and the client, for scripts and notebooks
├── docs/
└── scripts/check-layers.sh
```

### An app (garnish first; dsp-kitchen and lucem the same shape)

```text
garnish/
├── domain/                    ← Model: its own part
│   ├── model                  types and data model
│   ├── operations             its ops
│   └── formats                readers and writers
├── app/
│   ├── viewmodels/            one per screen: state, commands, offered inputs; headless tests
│   └── views/                 GPUI screens drawing those view-models
├── figures/                   figure styling
└── python/                    its Python package, re-exporting the core's
```

Gone from every app: its own server, command line, MCP host, pipeline format, script runner,
run-record format.

### The orchestrator: the runtime deployed for a lab

```text
orchestrator/
├── queue/                     many machines, scheduling
├── database/                  the lab's shared store (Postgres)
├── dashboard/                 the lab's web view
└── deploy/                    containers, compose, smoke test
```

### The template: generates only what an app owns

```text
template/
├── domain/   (model · operations · formats)
├── app/      (viewmodels · views)
├── python/
└── project files (CI, docs, licences)
```

## Where today's code goes

| Today (kitchen, on branches) | Goes to |
|---|---|
| kitchen-artifacts: content IDs, kinds | `contract` (types, results) |
| kitchen-artifacts: provenance, run keys | `contract` (run records) + `runtime/store` |
| kitchen-tool: describe / validate / run | `contract` (ops) + `runtime/tools` |
| kitchen-pipeline: types, ops, graph, catalog | `contract` |
| kitchen-pipeline: engine, domain trait, walks | `runtime/engine` |
| kitchen-pipeline: scripts (read, probe, review, worker) | `runtime/scripts` |
| kitchen-ui: tokens, palettes, fonts, icons; widgets | `interface/theme`, `interface/widgets` |
| kitchen-shell: frame, file picker, commands, search, keys | `interface/frame` |
| kitchen-shell: jobs, activity, environments, changes | `runtime/services` (the frame keeps their display) |
| kitchen-views: dock, viewers | `interface/views` |
| kitchen-views: video decoding, frame times, sync | `runtime` (data), viewers stay in `interface/views` |
| kitchen-code: editor, change marks | `interface/editor` |
| kitchen-code: header reading, running scripts | `runtime/scripts` |
| kitchen-agent: chat panel | `interface/chat` |
| kitchen-agent: MCP host | `runtime/server` |
| kitchen-compute, kitchen-net, kitchen-hub, kitchen-plugin | `runtime/compute`, `network`, `downloads`, `tools` |
| kitchen-orchestrate: store, db, pipeline, runner | the orchestrator on the core runtime (queue, database, dashboard) |
