# kitchen-engine

Runs a checked pipeline over the data an app walks, piece by piece: lucem's tiles of a slide,
dsp-kitchen's windows of a recording, garnish's one table. The app provides a `Domain` (its types,
its pieces, how it reads, trims and writes a piece, its own ops); the engine runs the ops over it,
with progress, cancel and a preview of one piece.

The pipeline format and its check are the contract's (`kitchen-contract`); script ops are
kitchen-scripts'. Split from kitchen-pipeline in phase 2, step 3
([runbook](../../docs/plans/0.0.10-standard.md)).
