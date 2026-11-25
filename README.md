# Strata

*Working name. The public name is chosen before anything is published (see
[decisions/0007](decisions/0007-working-name.md)).*

**Your data, kept the same way under every app.** Strata is a small protocol for storing records
so that any interface, including one an AI generates for a single person in a minute, reads and
writes the same data as every other.

Interfaces are becoming disposable: a person asks Claude Code or Gemini for the budget app, the
lab notebook or the booking screen they want, and gets it. What must not be disposable is the
data underneath. Today every app keeps its own, in its own shape, behind its own back end, and
your history is stuck in whichever app you picked. Strata standardizes the one layer everything
passes through, and nothing above it.

## The core, on one page

| | What it is | Why |
|---|---|---|
| **Records** | A typed document: a transaction, a booking, an analysis run. It names its schema (`neuro-kitchen.org/ledger/transaction@1`) and carries a JSON body | Any app can read what any other wrote |
| **Identity** | Stable IDs for records; content IDs (SHA-256) for anything whose bytes matter; IDs for people, apps and agents | Things can be referred to, verified and attributed |
| **History** | Records change by appending a change, never by overwriting. Each change says who made it, through which app or agent, when, and what came before | Two interfaces cannot silently disagree; every view (a balance, a summary) is computed from history; nothing is lost |
| **Permissions** | Grants: who (person, app, agent) may read or write which records, and which actions need a person's approval | Agents can act without being trusted with everything |
| **Access** | A handful of operations (hello, get, list, history, write, subscribe, blobs) over HTTP, MCP and local stdio | An agent, an app or a script connects the same way |

That is the whole standard. **Schemas** (what a transaction or a booking looks like) are published
by anyone, under their own domain, and evolve by adding fields; readers ignore what they don't
know. **Interfaces, workflows and business logic** are not standardized at all: they are where
products compete, or what people generate for themselves.

```text
   generated UIs · apps · agents · scripts          ← free: anyone, any language, any design
 ───────────────────────────────────────────────
   schemas: ledger/*, lab/*, shop/*, …              ← open: published by communities, evolve by adding
 ───────────────────────────────────────────────
   records · identity · history · permissions · access   ← Strata: small, slow to change
 ───────────────────────────────────────────────
   a store: on a laptop, a lab server, a hosted service   ← anyone can run one
```

## What it looks like

A record, as stored (one change in its history):

```json
{
  "record": "rec_01JDZ3K9Q4T7V2M8X5N6B1C0AA",
  "schema": "neuro-kitchen.org/ledger/transaction@1",
  "op": "create",
  "body": {
    "date": "2026-11-25",
    "amount": {"value": "-42.10", "currency": "USD"},
    "account": "rec_01JDZ2…",
    "payee": "Corner Grocery",
    "category": "food"
  },
  "prev": [],
  "by": "did:key:z6Mk…",
  "via": "app:budget-generated-by-claude",
  "at": "2026-11-25T18:04:11Z",
  "change": "sha256:3fa9…"
}
```

An agent connected over MCP sees tools like `strata.list`, `strata.get`, `strata.write`. Ask it
for "a budget view by category for November" and it builds the view from the same records every
other app sees. Ask it to "recategorize the grocery runs" and the write goes through the grants:
allowed, refused, or held for your approval.

## Where it is

| Part | State |
|---|---|
| Principles and decisions | Written: [spec/00-principles.md](spec/00-principles.md), [decisions/](decisions/) |
| Spec, parts 1–7 | First drafts: [spec/](spec/) |
| Core JSON Schemas | Next (written from the reference implementation) |
| Reference store (Rust, SQLite) with HTTP and MCP | Planned: [roadmap.md](roadmap.md) |
| Conformance tests | Planned |
| First domain: a personal and shared ledger | Planned: the pilot |
| neuro-kitchen (lab records, analysis runs) | Planned: kitchen and its apps implement Strata |

Read next: [the principles](spec/00-principles.md) · [prior art](prior-art.md) · [roadmap](roadmap.md) ·
[governance](GOVERNANCE.md).

## Licence

The specification and documents: [CC BY 4.0](LICENSE-DOCS.md). Schemas and code: Apache-2.0
([LICENSE-APACHE](LICENSE-APACHE)) or MIT ([LICENSE-MIT](LICENSE-MIT)), at your option.
