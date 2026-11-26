# 0. Principles

Strata exists because interfaces are becoming cheap and disposable, while the steps that process
data and the results they produce must stay comparable, traceable and shareable for years. Every
principle serves one goal: **a small, stable contract every app shares, which leaves everything
above it free to change.**

## 1. Standardize the waist, and nothing above it

The internet standardizes packets, not applications; Git standardizes commits, not workflows.
Strata standardizes how steps are described, how results are stored and how they link. It does
not standardize how an app reads its files, walks its data, computes on its device or draws its
screens.

**Test:** the contract fits on one page ([README](../README.md)).

## 2. Nothing enters the contract unless two apps need it

A feature joins the contract only when two apps (dsp-kitchen and lucem, garnish and the
orchestrator) need it and cannot do it in their own domain. Until then it is a namespaced type,
a parameter, or app code.

## 3. Add, never remove

Results are kept for years and cited in papers. The contract changes by adding: new types, new
optional fields, new calls. Readers ignore what they do not know. A breaking change is a new major
version that lives beside the old one.

## 4. Every result says what produced it

No result exists without its run record: the step and its version or hash, the parameters, the
inputs by content ID, who ran it and through what. "How was figure 3 made?" is always answerable
from records alone.

## 5. Records are appended, never edited

Results are immutable (named by their content). Run records, confirmations and curation decisions
are appended; a correction is a new run or a new decision, and the old one stays visible.

## 6. Shared behaviour is shared code

The runtime (running steps and pipelines, scripts, storage, the API) is one implementation every
app uses, not a specification each app reimplements. The contract is written down so others can
build their own; the apps use the reference.

## 7. Apps own their domain, nothing more

An app brings its types, its operations, its file formats and how it walks its data (windows,
tiles). It does not bring its own server, command line, MCP host, pipeline format or script runner.

## 8. Generated code runs only once confirmed

Steps from scripts, and pipelines, are checked (they load, they declare what they say) and
confirmed by a person, tied to their content hash. Any change asks again.

## 9. The reference implementation comes first

Each part of the spec is written from working code in kitchen and corrected by it. A part without
an implementation is a draft.

## 10. Easy for an AI to use correctly

Interfaces and steps will often be generated. So: few calls, self-describing steps and results,
types an agent can read, errors that say what to do.
