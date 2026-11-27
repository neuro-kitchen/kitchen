# kitchen-client

The runtime API's client side ([standard §6](../../standard/spec/06-runtime-api.md)):

- `calls`: the calls as data (`Request`, `Answer`, `Caller`), the messages every transport carries;
- `Runtime`: anything that answers them, with typed helpers (`ops`, `run`, `watch`, `wait`,
  `cancel`, `results`, `read`, `export`, `lineage`);
- `Local`: the in-process runtime an app's view-models use. Runs go to compute workers apart from
  the caller; the app's `Runner` (its engine over its domain, or a tool) does the work; outputs are
  kept in the store by reference with a run record; an identical run is recognized by its run key
  and reused (never for an op with effects, or a cancelled run's).

Phase 2, step 4a ([runbook](../../docs/plans/0.0.10-standard.md)); the server that carries the same
calls over WebTransport and Connect is step 4c–4d.
