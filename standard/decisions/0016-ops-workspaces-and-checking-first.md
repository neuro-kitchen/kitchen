# 0016. Ops, workspaces, and checking first

**Context.** Walking examples 01–04 through the spec ([walkthrough](../examples/walkthrough.md))
left twelve questions: four from the walk, eight the spec had carried as open. Deciding them showed
three things larger than any one answer: the standard's words did not match how neuro-kitchen
talks; ops need a home next to the data; and confirming scripts is worth doing well, later.

**Decision.**

- **Words.** An **op** is one operation (built in, a script, a tool); a **pipeline** is a graph of
  ops inside one app's workspace; a **recipe** is a pipeline saved as an op (dsp-kitchen's recipes
  are the model: exposed settings become its parameters, its version is its content hash); a
  **chain** joins apps and exists only on the orchestrator. "Step" and "composite step" leave the
  spec ([§2](../spec/02-ops.md), [§4.6](../spec/04-pipelines.md)).
- **Confirmation moves to v0.2.** Script ops and recipes run once **checked**; run records name the
  hash that ran. Confirming (whose confirmation counts, trusted maintainers, roles, agents never
  confirming) is written down as v0.2 ([§5](../spec/05-confirmation.md)).
- **Workspaces, like `.vscode`.** A folder next to the data holds an app's workspace (`.dspk/`,
  `.lucem/`): local settings and keys, `ops/`, `recipes/`, `pipelines/`, `runs/`. Ops are found
  there first, then in the person's configuration folder (where the app's settings and keys live:
  `~/.config/dspk-app/`), then the lab's through the orchestrator. Every one is a plain file,
  exported and imported as files ([§2.2](../spec/02-ops.md)).
- **Each app sees its own ops.** An app lists its ops, the shared ones, its tools' and its folders';
  another app's ops are ignored. Only the orchestrator knows every app's, and runs chains
  ([§4.6](../spec/04-pipelines.md)).
- **Which ops fit a screen** is the runtime's answer (`ops` with `fillable_by`), so a menu, a page
  and an agent agree; to be refined with use ([§6.1](../spec/06-runtime-api.md)).
- **"What was made from this?"** is lineage read forward (`results` with `from`), built only on
  written run records ([§3.4](../spec/03-results.md)).
- **Units come from the data.** A type names its axes and the kind of unit along each; the values
  (30 kHz, 0.25 µm) come from each result's metadata ([§1.5](../spec/01-types.md)).
- **No type inheritance** in this version ([§1.4](../spec/01-types.md)).
- **Every op declares a version**, a whole number starting at 1, bumped when its behaviour changes;
  scripts and recipes use their content hash ([§2.1](../spec/02-ops.md)).
- **Side effects are marked** (`effects`): never reused, never previewed, asking before each run
  unless allowed for the session ([§2.1](../spec/02-ops.md), [§3.3](../spec/03-results.md)).
- **A cancelled run's outputs** are kept and usable, marked *run incomplete*; running again starts
  from the beginning ([§3.3](../spec/03-results.md)).
- **No "for each" node**: batches and `many` ports ([§4.6](../spec/04-pipelines.md)).
- **Previews are each app's**: the contract keeps only that a preview runs on a piece, stores
  nothing and records nothing; which piece, which level, caching and cancelling are the app's, so
  it can optimize ([§4.4](../spec/04-pipelines.md)).

**Consequences.** The spec reads in the words people use. A recording's folder carries its ops,
recipes and history. Generated scripts run on a person's machine without a confirmation step until
v0.2, which is where a shared lab server needs it. The first contract version is smaller: no
confirmation, no preview protocol, no for-each, no inheritance.

**Alternatives.** Confirmation in the first version (safer on shared servers, slower to build and
to use while everyone works on their own machine). One shared `.kitchen/` folder for every app
(simpler, but an app would then list other apps' ops it cannot run).
