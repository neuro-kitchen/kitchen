# 0014. The standard lives in kitchen, beside its implementation

**Context.** Strata began as its own repository, so the contract could be read and implemented
apart from neuro-kitchen's code. But kitchen is its only implementation, and the next phase
([roadmap](../roadmap.md), phase 2) rewrites kitchen to follow it. With the spec in another
repository, every change to the contract is two pull requests that can drift apart, the JSON
Schemas live away from the types that generate them, and the examples can only be checked on
paper.

**Decision.** The standard lives in kitchen, under `standard/` (spec, decisions, examples,
architecture, roadmap, governance), with its history. The `strata` repository is archived with a
pointer here.

- A change to the contract and the code that implements it is one pull request.
- The schemas under `standard/schemas/` are generated from the contract crate, and a test fails
  when they differ (as kitchen-template does for its documents).
- Examples 01–04 become tests against the runtime as phase 2 builds it.
- Kitchen follows the standard first; apps move onto it after, so no app adapts to a version of
  kitchen from before the standard.
- Releases stay on `0.0.x`: 0.0.9 is the last kitchen from before the standard, 0.0.10 the first
  that follows it.

**Consequences.** One place to read and change the contract, checked by the same CI. The standard
is less visibly separate from one implementation; that matters only once others implement it.

**Alternatives.** Keep a separate repository and copy the schemas into kitchen (two sources that
drift); a git submodule (two pull requests still, and submodules confuse every tool). Splitting
the standard out again is the "before publication" step, under its public name, when there is a
second implementation to keep it neutral for.
