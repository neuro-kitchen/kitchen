# 0008. The standard is the pipeline contract

Replaces the scope of [0001](0001-narrow-waist.md).

**Context.** The first drafts standardized generic records, history and permissions: a data layer
for any app, illustrated with a ledger. That is not what neuro-kitchen needs. Its apps share, or
should share, one thing: how processing steps are described, how their results are stored, and how
steps and results link across apps.

**Decision.** The standard (the waist) is the pipeline contract: types, steps, results (with run
records), pipelines, and confirmation, plus the runtime API that interfaces, agents and other apps
call. Records, identity and history survive as parts of run records, not as a general database.
Money and other non-science domains are out of scope.

**Consequences.** The contract is grounded in code that exists (kitchen-pipeline, kitchen-artifacts,
kitchen-tool). It is narrower and easier to test (examples 01–04). Anyone outside neuroscience can
still use it for pipelines, but nothing is designed for them.
