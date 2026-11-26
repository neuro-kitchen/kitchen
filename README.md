# Strata

*Working name; the public name is chosen before anything is published
([decisions/0007](decisions/0007-working-name.md)).*

**The pipeline contract of neuro-kitchen.** Every neuro-kitchen app (dsp-kitchen, lucem, garnish,
neuro-convert, the orchestrator, and any interface someone generates) meets in one place: how a
processing step is defined, how its results are stored, and how steps and results link up. Strata
writes that place down, so the output of anyone's step can feed anyone else's, and every result can
say exactly what produced it.

Interfaces are becoming cheap: a scientist asks Claude for the plot, the screen or the button they
are missing, and gets it. What must not change with every interface is what is underneath: the
steps, the results and their history. Strata standardizes that, and nothing above it.

## The contract, on one page

| | What it is | In one line |
|---|---|---|
| **Types** | Names for what flows between steps and what is stored: `signal`, `image`, `labels`, `table`, `figure`, `region`…; apps add their own, namespaced (`dspk:sorting`) | A step's output can feed another's input when their types match |
| **Steps** | A processing step: its inputs and outputs (typed), its parameters, the margin it needs around a piece of data, and its code (built in, a script, a tool, or a whole pipeline) with that code's hash | Any app, script or tool describes what it does the same way |
| **Results** | Each output stored by content ID, with a run record: which step at which version, which parameters, which inputs, who ran it, through which app or agent | Any result can be traced back to the raw data; an identical run is recognized and reused |
| **Pipelines** | Steps connected by typed ports, as versioned data; run piece by piece (windows of a recording, tiles of a slide) by whichever app owns the data | A pipeline made in one app runs in another, or on a lab server |
| **Confirmation** | A step from a script, or a pipeline, runs only after it is checked and a person confirms it, tied to its content hash | Generated code is safe to share across a lab |

Plus a **runtime API**: the few calls an interface, an agent or a script makes to list steps, run
them, follow a run, and read results and their history, over HTTP, MCP or in-process.

What is **not** standardized: how each app reads its files or walks its data, what operations it
offers, how it draws anything. Those are where apps differ, and where people (and their agents)
are free.

```text
   interfaces: dsp-kitchen · lucem · garnish · generated ones · agents · notebooks   ← free
 ─────────────────────────────────────────────────────────────────────────────────
   each app's domain: its types, its operations, its formats, how it walks data      ← the app's
 ─────────────────────────────────────────────────────────────────────────────────
   runtime: runs steps and pipelines, scripts, stores results, serves the API        ← shared code
 ─────────────────────────────────────────────────────────────────────────────────
   contract: types · steps · results · pipelines · confirmation                       ← Strata
```

## Read next

- [Architecture](architecture.md): the layers, the apps' MVVM structure, the repository trees.
- [Principles](spec/00-principles.md), then the spec: [types](spec/01-types.md),
  [steps](spec/02-steps.md), [results](spec/03-results.md), [pipelines](spec/04-pipelines.md),
  [confirmation](spec/05-confirmation.md), [runtime API](spec/06-runtime-api.md),
  [versioning](spec/07-versioning.md).
- [Examples](examples/): a missing plot, a different interface, a pipeline as a button, and the
  hard cases (dsp-kitchen's windows, lucem's tiles, previews, curation).
- [Roadmap](roadmap.md): the standard first, then garnish and the orchestrator, then neuro-convert,
  then dsp-kitchen and lucem.
- [Decisions](decisions/), [prior art](prior-art.md), [governance](GOVERNANCE.md).

## Where it is

| Part | State |
|---|---|
| Contract and runtime API | Drafts, written from kitchen's code (kitchen-pipeline, kitchen-artifacts, kitchen-tool) |
| Reference implementation | kitchen: parts exist on unmerged branches, not built yet; to be merged into one contract |
| First apps on it | garnish and kitchen-orchestrate (planned) |

## Licence

Documents: [CC BY 4.0](LICENSE-DOCS.md). Schemas and code: Apache-2.0 ([LICENSE-APACHE](LICENSE-APACHE))
or MIT ([LICENSE-MIT](LICENSE-MIT)), at your option.
