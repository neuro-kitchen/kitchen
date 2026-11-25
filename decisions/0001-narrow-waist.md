# 0001. Standardize the narrow waist, and nothing above it

**Context.** Standards attract features. Every participant wants their concern in the core, and
the core grows until only large organizations can implement it (Solid's linked-data stack is the
cautionary example). Meanwhile interfaces are becoming generated per person, so what needs to be
shared is the data underneath, not how it is shown or worked with.

**Decision.** The core is five things: records, identity, history, permissions, access. What a
record means is a schema, published by anyone. Interfaces, workflows, queries beyond simple
listing, analytics and business rules are out of scope. A feature joins the core only when two
real products need it and cannot do it on top (principle 2).

**Consequences.** A store can be written in a weekend; an agent can use one from the operation
list alone. Some things people expect from a database (rich queries, joins, full-text search)
are views apps build, which costs them work. Pressure to add "just one more" will be constant;
this decision is the answer to point to.

**Alternatives.** A full application platform (accounts, UI components, queries): more useful on
day one, impossible to keep small. A pure file format (like plain-text accounting): simplest, but
no permissions, no concurrent writers, no agents.
