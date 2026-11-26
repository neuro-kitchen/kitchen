# 0009. One shared runtime, not a back end per app

**Context.** dsp-kitchen and lucem each built their own pipeline format and executor, server, MCP
host, command line, script runner, Python bindings and run records; garnish and the orchestrator,
generated from kitchen-template, started with their own too. Behaviour standardized only on paper
drifts; seven systems built twice drift faster.

**Decision.** Running steps and pipelines, scripts, storage of results and records, and the
runtime API are one implementation in the core (`runtime/`), used by every app. The contract is
also written down (this repository) so others can implement it; neuro-kitchen's apps use the
reference. Apps keep only their domain: types, operations, formats, how they walk their data.

**Consequences.** A fix lands in every app at once; Claude learns one API; a new app is a domain
and its screens. The migration is real work (see the roadmap) and the core's runtime must meet the
hardest app's needs (example 04).

**Alternatives.** Specification only, each app implementing it (as email): more independent, and
exactly the drift we have now.
