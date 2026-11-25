# 3. History

*Draft.*

Records change only by appending a **change**. A store keeps every change; the current state of
a record, and every view of many records, is computed from them.

## 3.1 A change

| Field | Meaning |
|---|---|
| `record` | The record it changes. |
| `schema` | The record's schema (repeated, so a change can be read alone). |
| `op` | `create`, `update` or `retract`. |
| `body` | The record's **whole** body after the change (not a patch). Absent for `retract`. |
| `prev` | The content IDs of the changes this one follows: `[]` for a create, usually one, two or more when it resolves a conflict. |
| `by` | Who is responsible ([§2.3](02-identity.md)). |
| `via` | The app or agent that made the call (optional when `by` made it directly). |
| `at` | When the writer made it (RFC 3339, the writer's clock). |
| `note` | Optional: why, in words ("recategorized by rule *groceries*"). |
| `change` | Its content ID: SHA-256 of the canonical JSON of all the fields above. |
| `sig` | Optional: `by`'s Ed25519 signature of `change`. |

Whole bodies rather than patches: a change can be read on its own, any reader can show a diff,
and nobody has to agree on a patch format. Bodies are small ([§1.3](01-records.md)); blobs are
not repeated.

## 3.2 Heads and conflicts

The changes of a record form a graph through `prev` (as commits do in Git). Its **heads** are
the changes nothing follows. One head: the record's state is that change's body. Two or more:
the record is **in conflict**, and stays readable: a reader gets every head and says so.

A conflict is resolved by a new change whose `prev` lists the heads it resolves. The protocol
does not merge bodies; an app, an agent or a person decides, and that decision is itself in the
history.

A store may refuse a write whose `prev` is not the current head (optimistic concurrency) and let
the writer retry; a store that accepts it records the conflict instead. Either is conforming;
the store says which in its `hello` ([§5](05-access.md)).

## 3.3 Order and views

A store gives every change it accepts a **sequence number**, increasing, its own. `list` and
`subscribe` use it as a cursor: "everything after 1042". Views (a balance, a monthly summary, a
table of runs) are computed by readers from records and changes; a store may keep views of its
own for speed, but they are never part of the protocol.

## 3.4 Signatures

A change may carry `sig`. A store checks every signature it receives and keeps it. Signed history
can be verified without trusting the store: copy it anywhere and check it. Unsigned changes are
attributed by the store (it vouches for `by`); signed ones by their author.

## 3.5 Open questions

- Should a store be allowed to compact old changes into a snapshot (with the originals archived)?
  Ledgers of small changes grow forever; audit wants every change.
- Clock skew: `at` is the writer's claim; the store's sequence is the order. Do views ever need
  the store's receive time too?
