# Schemas

JSON Schemas of the contract's documents, generated from the core's reference implementation
(principle 9), `contract/kitchen-contract`. A test fails when one differs from the types; regenerate
with `UPDATE_SCHEMAS=1 cargo test -p kitchen-contract --test schemas` and commit the result.

- `type.json`: a type's description ([spec §1](../spec/01-types.md))
- `op.json`: an op's description ([spec §2](../spec/02-ops.md))
- `run-record.json`, `result.json` ([spec §3](../spec/03-results.md))
- `pipeline.json` ([spec §4](../spec/04-pipelines.md))
- `confirmation.json` ([spec §5](../spec/05-confirmation.md)): with confirmation, in contract v0.2
