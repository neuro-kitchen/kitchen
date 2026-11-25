# 6. Schemas

*Draft.*

Schemas say what a record is. Strata does not define them (except a few of its own, below); it
defines how they are named, published, read and grown, so anyone can define them.

## 6.1 Naming

`<domain>/<path>@<major>`: a domain the publisher controls, a path, a major version.

- `neuro-kitchen.org/ledger/transaction@1`
- `neuro-kitchen.org/lab/run@1`
- `example-bakery.com/orders/order@2`

The domain says who answers for it; nobody needs a central registry to publish one. Schemas for a
domain (a ledger, a lab) never carry the protocol's own name, only their publisher's (see [decisions/0006](../decisions/0006-schema-ids-on-domains.md)).

## 6.2 Form and publishing

- A schema is a JSON Schema (draft 2020-12) document, published at
  `https://<domain>/<path>/<major>.json`, plus a short text saying what the record is for.
- Its content ID is recorded with every store that uses it, so a schema changed after
  publication is noticed.
- Stores keep copies of the schemas they use (`hello` lists them) and serve them, so a record is
  understandable even if the publisher's site disappears.

## 6.3 Growing a schema

Within a major version a schema only grows: new optional fields, wider ranges, new enum values
read as "other" by older readers. Readers ignore fields they do not know and keep them when they
write back ([§1.2](01-records.md)). A change that would make old records invalid, or old readers
wrong, is a new major version; both live side by side.

## 6.4 Mappings between schemas

Two communities will define a "transaction" differently. Strata does not pick one. A **mapping**
(`strata.dev/core/mapping@1`) is a record that says how to read one schema as another: field
renames, constants, simple conversions. Apps and agents use mappings to show records of
both kinds together. Convergence happens in practice, as one schema wins users, not by committee.

## 6.5 Strata's own schemas

The few records the core itself needs, all under `strata.dev/core/`: `grant@1`, `pending@1`,
`approval@1`, `mapping@1`, `schema@1` (a schema a store holds). *(The domain is a placeholder
until the public name is chosen.)*

## 6.6 Open questions

- How small can mappings stay before they become a programming language? Renames, constants and
  unit conversions may be enough; anything more is an app's code.
- Should a store validate bodies against their schemas on write (strict), or accept and flag?
  Strict by default seems right; flagging helps importing messy data.
