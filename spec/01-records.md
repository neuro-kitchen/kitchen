# 1. Records

*Draft. Written ahead of the reference implementation; it will correct this.*

A **record** is one thing a person or a program cares about: a transaction, an account, a
booking, an analysis run, a figure. It has a stable ID, a schema, and a body that changes over
time through its [history](03-history.md).

## 1.1 What a record has

| Field | Meaning |
|---|---|
| `record` | Its ID: `rec_` followed by a ULID (26 characters, sortable by creation time), made by whoever creates it. Stable for the record's whole life. |
| `schema` | What it is: a schema ID, `<domain>/<path>@<major>` ([§6](06-schemas.md)). Fixed at creation. |
| `body` | Its content: a JSON object that validates against the schema. |

Everything else (who changed it, when, what came before) belongs to its changes, not to the
record itself.

## 1.2 Bodies

- A body is a JSON object (RFC 8259), UTF-8.
- Amounts that must be exact (money, measurements quoted to a stated precision) are strings of
  decimal digits, not JSON numbers: `{"value": "-42.10", "currency": "USD"}`. Floating point
  loses cents.
- Times are RFC 3339 with an offset (`2026-11-25T18:04:11Z`); dates without a time are
  `YYYY-MM-DD`.
- A reference to another record is its ID (`"account": "rec_01JD…"`). A reference to bytes is a
  content ID (`"file": "sha256:…"`, [§2](02-identity.md)).
- **Unknown fields are kept and ignored.** A reader that does not know a field leaves it as it is
  when it writes the record back. This is what lets schemas grow without breaking older apps.

## 1.3 Large content

Bodies stay small (a store may refuse bodies over 1 MiB). Files, images, recordings and tables
are **blobs**: stored separately, named by their content ID, and referenced from a body. A blob
is immutable; a new version is a new blob and a change to the record that points at it.

## 1.4 Deleting

A record is never erased from history by the protocol. It is **retracted**: a change with
`op: "retract"` marks it gone, and views leave it out. Erasing data for legal reasons (a deletion
request, a court order) is the store's duty, done outside the protocol, and leaves a tombstone
change saying that history was cut, by whom and why.

## 1.5 Open questions

- Should a record be able to change its schema's major version in place (a migration change), or
  must it be a new record pointing at the old one?
- A size limit for bodies: 1 MiB, or smaller to keep stores cheap?
