# 6. The runtime API

*Draft. What interfaces, agents, scripts and other apps call. The same calls in-process (an app's
own view-models), over HTTP (a generated interface, another machine), over MCP (an agent) and over
stdio (tools, tests).*

## 6.1 Calls

| Call | Takes | Gives |
|---|---|---|
| `hello` | the caller's contract version and capabilities | the runtime's version, capabilities, its apps (domains) |
| `types` | — | every type: shared and the apps', with descriptions |
| `ops` | optional: a type an input should accept, `fillable_by` (a screen's offers: only ops whose required inputs they can all fill), a family, a search text | op descriptions (this app's, the shared ones, its tools', its folders' scripts and recipes) |
| `op.add` | a script (file or text) | its description and state (Unreadable … Checked) |
| `op.install` | a tool op that is not available | progress until it is (weights, a runtime), or why not |
| `pipeline.check` | a pipeline | its problems, or its plan (order, margin) |
| `pipeline.save_as_recipe` | a pipeline, the parameters to expose | the recipe |
| `run` | an op or pipeline, what fills its sources (result IDs, or an app's source description), parameters, run options (device, piece size, threads); or a list of fillings, one run each | a run ID, or a batch ID and its runs |
| `run.watch` | a run ID | progress events until it ends |
| `run.cancel` | a run ID | — |
| `preview` | an op or pipeline, one piece, parameters | the outputs for that piece (not stored; how the app previews is its own, §4.4) |
| `results` | filters: type, op, run, since, `from` (a source or result: everything whose lineage reaches it) | results |
| `result.read` | a result ID; for an array, optionally a region (a tile, a window, a level) | its bytes, or that region's, read by the app's domain |
| `result.export` | a result ID, a path | the file written there, its run record beside it |
| `lineage` | a result ID | the runs and results it came from, back to the sources |
| `offers` / `pin` | a screen | what it offers; pin an op there as a command |

Errors say what to do: "this pipeline uses `script:boost`, which fails its check at line 12;
open it in Code", or "`convert:nwb` is not an op of dsp-kitchen; run this as a chain on the
orchestrator".

Planned for v0.2: `op.confirm` (an op and its hash → Confirmed, or why not; only people),
[§5.4](05-confirmation.md).

## 6.2 Transports

| Transport | Used by | Form |
|---|---|---|
| In-process | an app's own view-models | the client library's calls |
| HTTP | generated interfaces, other machines, the orchestrator | `POST /v0/<call>` with JSON; `run.watch` as Server-Sent Events; results as `GET /v0/result/<id>` |
| MCP | agents (Claude Code, others) | one tool per call; types and op descriptions as resources |
| stdio | tools, tests | JSON lines |

## 6.3 Who is calling

Every call is made by someone (`by`, a person) through something (`via`: an app, an interface, an
agent). The runtime records both on every run. What each may do (run, delete; confirm from v0.2) is
the runtime's setting: on a laptop, the person can do everything, and so can their agents, `via`
recorded; on a lab server, the orchestrator's roles apply.

## 6.4 Open questions

- None left from the first draft (previews are each app's: decision 0016).
