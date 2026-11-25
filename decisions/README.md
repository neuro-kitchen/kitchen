# Decisions

One page per decision: what was decided, why, and what was given up. Append-only: a decision is
changed by a new one that replaces it, never by editing the old. When someone asks "why don't we
just add X?", the answer is usually here.

| # | Decision | State |
|---|---|---|
| [0001](0001-narrow-waist.md) | Standardize records, identity, history, permissions and access, and nothing above | Accepted |
| [0002](0002-append-only-whole-bodies.md) | History is append-only; a change carries the whole body | Accepted |
| [0003](0003-json-jcs-sha256.md) | JSON, canonicalized by RFC 8785, named by SHA-256 | Accepted |
| [0004](0004-json-schema-not-rdf.md) | Schemas are JSON Schema, not RDF or a new language | Accepted |
| [0005](0005-agents-are-actors.md) | Agents are actors with grants; actions that matter wait for approval | Accepted |
| [0006](0006-schema-ids-on-domains.md) | Schema IDs live on their publisher's domain | Accepted |
| [0007](0007-working-name.md) | "Strata" is a working name until publication | Accepted |

Format: **Context** (what forced a choice), **Decision**, **Consequences** (good and bad),
**Alternatives** considered.
