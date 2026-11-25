# 7. Versioning

*Draft.*

## 7.1 One version for the whole spec

The spec has one version, `MAJOR.MINOR`. Minor versions add (operations, optional fields,
capabilities); majors may remove or change meaning, and are rare. Until the first public
release the version is `0.x`, and anything may change.

## 7.2 The handshake

Every connection starts with `hello` ([§5](05-access.md)): each side states its version and its
**capabilities**, named features beyond the minimum (`signatures`, `approvals`, `blobs`,
`subscribe`, `mappings`). Each side uses only what both have. A store without approvals refuses
writes that would need one, saying so; it does not silently apply them.

## 7.3 Compatibility

- A reader of version `N.m` reads everything written by `N.k` for any `k`.
- A store of `N.m` accepts writes from clients of `N.k`, ignoring what it does not know (and
  keeping it).
- A new major version comes with a published conversion, and stores serve both for at least
  two years.

## 7.4 Conformance

A conformance suite (planned) tests a store and a client against the spec: each operation, each
error, conflicts, approvals, export and import. "Strata-compatible" means passing it at a given
version, with its capabilities listed.
