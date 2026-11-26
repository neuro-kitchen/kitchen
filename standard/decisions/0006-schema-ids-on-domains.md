# 0006. Schema IDs live on their publisher's domain

**Context.** Schema IDs end up inside every record ever written; they are the hardest thing to
change later. The protocol's name may change before publication (0007), and a central registry
would make us the gatekeeper of every community's schemas.

**Decision.** A schema ID is `<domain>/<path>@<major>`, on a domain its publisher controls
(`neuro-kitchen.org/ledger/transaction@1`). Schemas for a domain of use never contain the
protocol's name. Only the core's own few schemas live under the protocol's domain.

**Consequences.** Anyone can publish without asking; the ID says who answers for it; renaming the
protocol does not touch stored ledgers or lab records. A publisher losing its domain leaves IDs
that still work (stores keep copies) but can no longer be updated by them.

**Open.** The neuro-kitchen domain used in examples must be one the organization controls before
anything is stored with it.

**Alternatives.** A central registry (like npm for schemas): discoverable, but a gatekeeper and a
single point of failure. Bare names (`transaction`): collide immediately.
