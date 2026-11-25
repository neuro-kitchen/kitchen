# Prior art

What others built for "data that outlives its apps", and what Strata takes from each. The short
version: small formats last (email, iCalendar, plain-text accounting); rich data stacks without a
popular app stall (Solid); a small core with open schemas and one app people want grows
(AT Protocol).

| | What it standardizes | What happened | What we take | What we avoid |
|---|---|---|---|---|
| **Email, iCalendar, vCard** | Small record formats | Universal for decades | Small, boring, readable formats last | No history, no permissions: each server does its own |
| **Plain-text accounting** (ledger-cli, hledger, Beancount) | A text format for transactions | A loyal community; many tools share one file | Ledger schemas: double entry, commodities, exact decimals; an importer and exporter from day one | One file, one writer: no concurrent apps, no agents, no permissions |
| **Git** | Content-addressed objects and a history graph | Universal for code | Content IDs; `prev` links; heads and merges as history | Whole-repository sync; a model only developers enjoy |
| **Solid** | Personal data pods, apps separate from data, linked data (RDF) | Little adoption | The goal itself | RDF and its learning curve; launching without an app people want |
| **AT Protocol** (Bluesky) | Personal data repositories, signed records, "lexicons" (open schemas) | Growing; several apps share data | Records in per-person repositories; open schemas on domains; signed history | Its own schema language; a design centred on public social data |
| **Local-first** (Ink & Switch, Automerge) | Data on the device, synced, merged by CRDTs | Influential; used inside products | A store can be a file on a laptop; export and import as a right | Merge semantics fixed in the protocol (decision 0002) |
| **HL7 FHIR** | Health records as typed resources over REST, with profiles and extensions | Required by regulators; widely implemented | Profiles and extensions instead of forks; must-ignore unknown fields | A huge resource catalogue in the core |
| **GA4GH** (genomics) | Small APIs: tools, workflows, data access | Adopted by large institutions | Small services, each standardized separately | Domain-specific from the start |
| **W3C PROV, RO-Crate** | Provenance: what produced what, packaged with data | Standard in research data | Changes carry `by`, `via`, `prev`; an export can be mapped to PROV | Their vocabulary in the core |
| **Open banking** (PSD2, FDX) | Account and payment data over APIs, under consent | Mandated in many countries | Grants with scope and consent; data access as a right | Rules written for one regulated industry |
| **MCP** | How agents call tools and read resources | Fast adoption by AI assistants | The agent transport: operations as tools, schemas as resources | Being only an agent protocol: Strata is the data the tools reach |

## Gaps none of them fill together

1. **Many generated interfaces writing the same records**, each attributed, with conflicts visible
   instead of lost.
2. **Agents as first-class actors**, with grants and approvals in the protocol, not in each app.
3. **One small core across domains** (money, bookings, lab results) with schemas open to anyone.
