# 2. Identity

*Draft.*

Three kinds of identity: of **records** (which thing), of **content** (which bytes), and of
**actors** (who). Each is a plain string.

## 2.1 Record IDs

`rec_` + a ULID: made by the writer, unique without coordination, sortable by creation time.
Stores reject a create for an ID they already hold.

## 2.2 Content IDs

`sha256:` + 64 lowercase hex digits, the SHA-256 of the bytes. For JSON (a change, a schema), the
bytes are the canonical form of RFC 8785 (JSON Canonicalization Scheme), so the same content
always has the same ID whoever serialized it.

Content IDs name blobs, changes and published schemas. The same value as `sha256sum` gives for a
file, so anyone can check one without Strata software.

## 2.3 Actors

Anyone who makes a change or holds a grant: a **person**, an **app** or an **agent**.

| Kind | ID | Example |
|---|---|---|
| Person | a DID (`did:key:…` for a key held by the person; `did:web:…` for one published at a domain) | `did:key:z6MkhaXgBZD…` |
| App | `app:` + a name the store registers, plus its key | `app:budget-generated-by-claude` |
| Agent | `agent:` + a name, acting for a person under a grant | `agent:claude-code@laptop` |

- A change records both **who** (`by`: the person or app responsible) and **through what**
  (`via`: the app or agent that made the call). "Ana, through her Claude Code agent" is
  `by: did:key:…ana`, `via: agent:claude-code@laptop`.
- Keys are Ed25519. Changes may be signed by `by`'s key ([§3.4](03-history.md)).
- A store maps its own accounts (a login, a team) to actor IDs; the protocol does not define
  accounts or log-in.

## 2.4 Open questions

- Is `did:key` the right default for people, or should the first version use a store-issued ID
  and add DIDs later? DIDs make moving between stores possible; they also add a concept.
- Agent identity: one ID per agent program, or per agent and device (`agent:claude-code@laptop`)?
  Approvals and audit want the second.
