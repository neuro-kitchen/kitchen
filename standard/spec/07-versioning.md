# 7. Versioning

*Draft.*

## 7.1 One version for the contract

The contract (types, steps, results, pipelines, confirmation, runtime API) has one version,
`MAJOR.MINOR`. It replaces the separate numbers kitchen has today (the tool protocol's, the graph
format's). Minor versions add; majors may change meaning, are rare, and come with a conversion.
Until the first public release it is `0.x` and anything may change; it is not called `1.0` until
dsp-kitchen and lucem run on it.

## 7.2 Handshake

`hello` ([§6](06-runtime-api.md)) states each side's version and capabilities (`scripts`,
`composites`, `preview`, `pins`, `lineage-export`; later `live`). Each uses only what both have; a missing
capability is refused with the reason, never silently skipped.

## 7.3 Compatibility

- Pipelines, run records and results written under `N.k` are read by every `N.m`.
- A stored pipeline names its contract version; the runtime converts older majors on read.
- App versions and the contract version are separate: an app states which contract versions its
  domain supports.

## 7.4 Conformance

A conformance suite (with the reference implementation) checks a runtime and a client: every call,
every error, checking, confirmation, reuse by run key, lineage, preview, and the hard cases (pieces
and margins give the whole's result). "Strata-compatible" (or its successor's name) means passing
it at a version.
