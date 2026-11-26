# 0012. Types are shared names or namespaced app names

Replaces [0006](0006-schema-ids-on-domains.md) for the contract's types.

**Context.** 0006 put schema IDs on publishers' web domains, for a general data layer. The contract
needs names for types that appear in every step and result: short, readable in a pipeline file,
stable.

**Decision.** Shared types have bare names defined by the contract (`signal`, `image`, `table`,
`figure`…). App types are `<namespace>:<name>` (`dspk:sorting`, `lucem:cells`); the namespace is the
app or project. No central registry.

**Consequences.** Pipelines stay readable; renaming the standard does not touch stored results.
Namespace collisions between outside projects are possible; a list of known namespaces in this
repository is enough until it is not.
