# 0015. What lucem and dsp-kitchen need from the contract

**Context.** Before kitchen is restructured (phase 2), lucem 0.0.6 and dsp-kitchen 0.0.13 were read
without changing either, and every part of their back ends was placed in the architecture
([docs/plans/0.0.10-gaps.md](../../docs/plans/0.0.10-gaps.md)). Ten things they do had no place,
or only part of one, in the spec.

**Decision.** All ten recommendations are accepted, and the spec now says each:

1. **Batch runs**: `run` takes a list of fillings, one run each, under a batch ID; reading several
   files as one is a domain's source description ([§4.1](../spec/04-pipelines.md), [§6.1](../spec/06-runtime-api.md)).
2. **Writing to a path is an export**: outputs always go to the store; `result.export` writes one
   where a person asks, with its record ([§3.5](../spec/03-results.md)).
3. **`finish`**: a step may say how its pieces' outputs are joined after the last piece; otherwise
   they are joined in piece order ([§2.1](../spec/02-steps.md), [§4.3](../spec/04-pipelines.md)).
4. **Run options**: device, piece size, read-ahead, threads, upload mode are options of a run, never
   part of a pipeline; recorded in the run's `environment`, left out of the run key ([§3.2](../spec/03-results.md)).
5. **Live sources** are not in this version; a later capability `live` ([§4.6](../spec/04-pipelines.md), [§7.2](../spec/07-versioning.md)).
6. **Large results are read by region**: `result.read` takes a tile, window or level for arrays,
   answered by the app's domain; huge results are kept by reference ([§3.5](../spec/03-results.md), [§6.1](../spec/06-runtime-api.md)).
7. **Availability**: a step says whether it can run here and why not; `step.install` for tool
   steps ([§2.1](../spec/02-steps.md), [§6.1](../spec/06-runtime-api.md)).
8. **Metrics**: optional time and bytes per step in run records, never in the run key ([§3.2](../spec/03-results.md)).
9. **Pyramid levels**: part of how a source is filled, so recorded; a preview may read a coarser
   level ([§4.1](../spec/04-pipelines.md), [§4.3](../spec/04-pipelines.md), [§4.4](../spec/04-pipelines.md)).
10. **Display is not processing**: views and their styles leave the pipeline for the screen's
    view-model, which names a pipeline output ([§4.1](../spec/04-pipelines.md)).

**Consequences.** Both apps' pipelines (four formats between them) fit one format; their
`read`/`write` nodes become sources and exports, their View nodes move to view-models, their
execution settings become run options. Two answers were left out on purpose: live runs (5) and
streamed previews (still open in §6.4).

**Alternatives.** Keeping `read`/`write` and execution settings in the pipeline (as dsp-kitchen
does) ties a pipeline to one machine's paths and device; the contract wants a pipeline to run
anywhere.
