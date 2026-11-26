# 0004. Schemas are JSON Schema, not RDF or a new language

**Context.** Schemas are where communities define their records. If writing one requires learning
an ontology language, communities will not write them; if every app invents its own, records will
not be shared.

**Decision.** A schema is a JSON Schema (draft 2020-12) document plus a short text. Meaning beyond
structure (that `amount` is money, that two schemas describe the same thing) goes in descriptions
and in mapping records (spec §6.4), not in a semantic-web layer.

**Consequences.** Developers and AI models already read and write JSON Schema; validators exist in
every language. Fine-grained semantics (linking a field to a shared vocabulary) are weaker than in
RDF; mappings carry the practical part of that.

**Alternatives.** RDF / JSON-LD (Solid, schema.org): rich semantics, and a reason Solid stayed
niche. AT Protocol lexicons: close to this, but their own language. A new schema language: no.
