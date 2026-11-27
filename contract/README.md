# contract/

The waist of the architecture: types, ops, results and run records, pipelines, and checking, as
the [standard](../standard/README.md) writes them. Depends on nothing of ours (serde and schemars
only), does no I/O and draws nothing.

Empty until phase 2, step 2 ([runbook](../docs/plans/0.0.10-standard.md)): `kitchen-contract` is
then born from kitchen-pipeline's and kitchen-artifacts' types, merged into one type system, one op
description, one run record and one pipeline format, with the JSON Schemas generated into
`standard/schemas/`.
