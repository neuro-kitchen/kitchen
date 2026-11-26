# 0007. "Strata" is a working name until publication

**Context.** "Strata" is widely used: an identity company selling an AI agent gateway over MCP
(close to a pilot we are considering), two blockchain protocols, a data format, an embedded
database for agents, and the package name `strata` on crates.io, PyPI and npm. Using it publicly
invites confusion and trademark trouble.

**Decision.** Use "Strata" for the repository and in conversation. Before publishing anything
(packages, a site, the spec as a standard): choose the public name, check package registries,
domains and trademarks (USPTO, EUIPO), and rename. Keep the name out of what is costly to change:
schema IDs (0006), field names, wire paths beyond a single prefix (`/strata/v0/`, one line).

**Consequences.** A rename touches the README, the spec's prose, the core schemas' domain and the
package names, all cheap before publication.
