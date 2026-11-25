# 0. Principles

Strata exists because interfaces are becoming cheap and disposable while data is not. Every
principle below serves one goal: **a small, stable layer that any interface can rely on, which
leaves everything above it free to change.**

## 1. Standardize the narrow waist, and nothing above it

The internet standardizes packets, not applications; Git standardizes its objects, not
workflows; email standardizes messages, not mail clients. Strata standardizes records, identity,
history, permissions and access. It does not standardize what a transaction is (that is a
schema), how a budget looks (that is an interface) or how a lab runs its analyses (that is a
workflow).

**Test:** the core fits on one page ([README](../README.md)). If it stops fitting, something
went in that should have stayed out.

## 2. Nothing enters the core unless two real products need it

A feature goes into the core only when two products that exist, built by different people or at
least for different domains, need it and cannot do it as a schema or a convention on top. Until
then it lives in a schema, an extension or a product.

## 3. Add, never remove

Formats people store data in must be readable for decades. The core and every schema change by
adding: new optional fields, new operations, new record types. Readers ignore what they do not
know. A breaking change is a new major version that lives beside the old one, never a
replacement.

## 4. The data outlives every app

No record depends on the app that wrote it to be understood: it names its schema, the schema is
published, and the history says who wrote what. Deleting an app loses nothing.

## 5. History, not state

A store keeps every change. Current state, balances and summaries are views computed from
history. This makes concurrent edits visible instead of lost, makes audits trivial, and lets a
new interface rebuild any view it wants.

## 6. Agents are actors, not super-users

People, apps and agents each have an identity. Every change records who made it and through
what. An agent gets grants like anyone else, narrowed to what it needs, and the actions that
matter wait for a person's approval.

## 7. Boring technology

JSON, HTTP, SHA-256, Ed25519, JSON Schema, MCP. Nothing a developer has to learn before they
start. Clever encodings, new cryptography and custom query languages stay out.

## 8. The reference implementation comes first

Each part of the spec is written from working code and corrected by it, not the other way
round. A spec section without an implementation is a draft.

## 9. Runs anywhere, owned by whoever runs it

A store can be a file on a laptop, a server in a lab or a hosted service. The protocol is the
same; moving between them is an export and an import. Nobody needs our permission or our
servers to use Strata.

## 10. Easy for an AI to use correctly

Interfaces will often be generated. So: few operations, self-describing records, schemas an
agent can fetch and read, errors that say what to do, and nothing that only works if you read
the spec carefully.
