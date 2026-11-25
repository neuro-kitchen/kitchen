# 5. Access

*Draft.*

Seven operations, the same over every transport. Any app, agent or script that can call them
can do everything Strata offers.

## 5.1 Operations

| Operation | Takes | Gives |
|---|---|---|
| `hello` | the caller's spec version and capabilities | the store's version, capabilities, conflict policy ([§3.2](03-history.md)), its schemas in use |
| `get` | a record ID | its current body (or every head, when in conflict) and its latest change |
| `list` | a schema pattern; optionally `since` (a sequence number), a simple filter on body fields, `limit` | records and a cursor for the next page |
| `history` | a record ID | its changes, in order |
| `write` | one or more changes | for each: accepted (its sequence number), pending approval (the pending record's ID), or refused (why, and what to do) |
| `subscribe` | a schema pattern and a cursor | a stream of changes as they are accepted |
| `blob` | `put` bytes / `get` a content ID | the content ID / the bytes |

A `write` of several changes is all-or-nothing: either every change is accepted (or held for
approval) or none is.

Errors say what to do, for a person and for an agent reading them: "refused: this agent may write
`ledger/transaction` only for account *Groceries*; ask the owner for a wider grant", not
"403".

## 5.2 Transports

| Transport | Use | Form |
|---|---|---|
| **HTTP** | apps, servers, anything | `POST /strata/v0/<operation>` with a JSON body; `subscribe` as Server-Sent Events; blobs as `PUT` / `GET /strata/v0/blob/<content-id>` |
| **MCP** | AI agents | one tool per operation (`strata.get`, `strata.list`, …), schemas as MCP resources so an agent can read what a record means |
| **stdio** | local programs, tests | JSON lines: one request, one answer |

Authentication is the transport's (HTTP: OAuth 2 bearer tokens or a store's own keys; MCP: the
host's; stdio: the local user). The protocol only needs to know which actor is calling.

## 5.3 Moving a store

`export` is `list` of everything plus `history` of everything plus the blobs, as a folder of JSON
lines and files. `import` is `write` of that history into another store. Signed changes keep
their signatures; unsigned ones are re-attributed by the new store, which records that it imported
them. A person can always take their data and go.

## 5.4 Open questions

- `list` filters: equality on body fields and a `since` cursor may be enough; anything richer is a
  view an app computes.
- Should `subscribe` over MCP be notifications, or polling with a cursor, until MCP's streaming
  settles?
