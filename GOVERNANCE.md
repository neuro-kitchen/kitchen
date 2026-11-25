# Governance

## Now (before publication)

- The maintainers of the neuro-kitchen organization decide. Every decision that shapes the core
  is written in [decisions/](decisions/) before it is merged.
- Changes come as pull requests. A change to the core cites the two products that need it
  (principle 2). Anything else goes in a schema or an extension.

## After publication

- **The core** (records, identity, history, permissions, access) changes by proposal: a pull
  request adding a decision record, open for comment for at least two weeks, with a reference
  implementation and conformance tests before it is accepted.
- **Schemas** belong to their publishers (decision 0006). This repository keeps only the core's
  own schemas and, as examples, the pilot's. Communities may list theirs in a directory here; the
  listing is not an endorsement.
- **Names and marks**: the public name and any compatibility mark ("Strata-compatible", or its
  successor) are used for implementations that pass the conformance suite at a stated version.
- When outside implementations exist, maintainers include at least one person from outside
  neuro-kitchen; the goal is a neutral home for the spec once it is adopted beyond us.
