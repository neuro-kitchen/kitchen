# 6. The runtime API

*Draft. What interfaces, agents, scripts and other apps call. The same calls in-process (an app's
own view-models), over HTTP (a generated interface, another machine), over MCP (an agent) and over
stdio (tools, tests).*

## 6.1 Calls

| Call | Takes | Gives |
|---|---|---|
| `hello` | the caller's contract version and capabilities | the runtime's version, capabilities, its apps (domains) |
| `types` | — | every type: shared and the apps', with descriptions |
| `steps` | optional: a type an input should accept, a family, a search text | step descriptions |
| `step.add` | a script (file or text) | its description and state (Unreadable … Checked) |
| `step.confirm` | a step and its hash | Confirmed, or why not (only people) |
| `pipeline.check` | a pipeline | its problems, or its plan (order, margin) |
| `pipeline.save_as_step` | a pipeline, the parameters to expose | the composite step |
| `run` | a step or pipeline, what fills its sources (result IDs, or an app's source description), parameters | a run ID |
| `run.watch` | a run ID | progress events until it ends |
| `run.cancel` | a run ID | — |
| `preview` | a step or pipeline, one piece, parameters | the outputs for that piece (not stored) |
| `results` | filters: type, step, run, since | results |
| `result.read` | a result ID | its bytes (or a handle for large ones) |
| `lineage` | a result ID | the runs and results it came from, back to the sources |
| `offers` / `pin` | a screen | what it offers; pin a step there as a command |

Errors say what to do: "this pipeline uses `script:boost`, checked but not confirmed; confirm it in
Pipeline › Scripts, or ask its author".

## 6.2 Transports

| Transport | Used by | Form |
|---|---|---|
| In-process | an app's own view-models | the client library's calls |
| HTTP | generated interfaces, other machines, the orchestrator | `POST /v0/<call>` with JSON; `run.watch` as Server-Sent Events; results as `GET /v0/result/<id>` |
| MCP | agents (Claude Code, others) | one tool per call; types and step descriptions as resources |
| stdio | tools, tests | JSON lines |

## 6.3 Who is calling

Every call is made by someone (`by`, a person) through something (`via`: an app, an interface, an
agent). The runtime records both on every run. What each may do (run, confirm, delete) is the
runtime's setting: on a laptop, the person can do everything and their agents everything but
confirm; on a lab server, the orchestrator's roles apply.

## 6.4 Open questions

- `result.read` for very large results (a 200 GB recording): a handle the app's domain reads in
  pieces, rather than bytes over the API?
- Should `preview` stream (a view updating while a slider moves) or answer once per call?
