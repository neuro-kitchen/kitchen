# contract/

The waist of the architecture: types, ops, results and run records, pipelines, and checking, as
the [standard](../standard/README.md) writes them. Depends on nothing of ours (serde and schemars
only), does no I/O and draws nothing.

- [`kitchen-contract`](kitchen-contract/): types and formats, ops, the catalog, pipelines and
  their check, content ids, run records and run keys. Born in phase 2, step 2, from
  kitchen-pipeline's and kitchen-artifacts' types, merged; the JSON Schemas in
  [`standard/schemas/`](../standard/schemas/) are generated from it, and the standard's examples
  ([`standard/examples/fixtures/`](../standard/examples/fixtures/)) are its tests.
