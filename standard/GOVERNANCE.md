# Governance

## Now (before publication)

- The maintainers of the neuro-kitchen organization decide. Every decision that shapes the core
  is written in [decisions/](decisions/) before it is merged.
- Changes come as pull requests. A change to the contract cites the two apps that need it
  (principle 2). Anything else stays in an app's domain.

## After publication

- **The contract** (types, ops, results, pipelines, confirmation, the runtime API) changes by proposal: a pull
  request adding a decision record, open for comment for at least two weeks, with a reference
  implementation and conformance tests before it is accepted.
- **App types** belong to their namespaces (decision 0012). This standard defines the shared
  types and keeps a list of known namespaces; a new shared type needs two apps (principle 2).
- **Names and marks**: the public name and any compatibility mark ("Strata-compatible", or its
  successor) are used for implementations that pass the conformance suite at a stated version.
- When outside implementations exist, maintainers include at least one person from outside
  neuro-kitchen; the goal is a neutral home for the spec once it is adopted beyond us.
