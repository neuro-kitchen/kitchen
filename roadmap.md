# Roadmap

Each phase ends with something people use. Dates are left out until the reference
implementation shows the pace.

## Phase 0: written (now)

- The README, principles, spec drafts (records, identity, history, permissions, access, schemas,
  versioning), decisions, prior art, governance.
- **Done when:** the README explains the core on one page, and reviewers (two people outside the
  project) can say what a store does without asking.

## Phase 1: the reference store

- A store in Rust: SQLite underneath; the seven operations; HTTP, MCP and stdio; signatures;
  grants and approvals; export and import.
- The core's JSON Schemas, written from it; the spec corrected by it.
- A conformance suite, run against the reference store.
- A small Python and TypeScript client (agents and generated apps reach for these first).
- **Done when:** a store runs on a laptop with one command, and Claude Code, given only the MCP
  connection, builds a working interface over it.

## Phase 2: the pilot domain, a ledger

- Schemas: account, transaction (double entry, exact decimals, commodities), category, budget.
  Taken from what plain-text accounting already worked out.
- Importers: CSV and OFX bank exports; an exporter to hledger / Beancount (nothing is locked in).
- Shared ledgers: a household or a small team, each person's agent with its own grants
  ("categorize, never move money", "any write over $500 needs approval").
- Demonstration: three different interfaces (a generated web page, a terminal view, a
  spreadsheet-like one) over the same records.
- **Done when:** ten people use it weekly for a month, and the "very disappointed if it went away"
  answer reaches 40 %.

## Phase 3: neuro-kitchen

- Schemas: `lab/artifact`, `lab/run`, `lab/recording`, `lab/annotation`, from kitchen-artifacts,
  the tool protocol and kitchen-pipeline's run reports.
- kitchen implements a store client; the orchestrator becomes a Strata store for runs and their
  provenance; dsp-kitchen and lucem write their results as records.
- Script confirmations (kitchen-pipeline's `approvals.json`) become grants and approvals.
- **Done when:** a lab can answer "which version of which script, with which settings, produced
  figure 3" from records alone, and an agent can build a new view of a lab's runs.

## Phase 4: the agent gateway

- A product over the store: every tool an agent can call goes through grants and approvals, with
  a readable history of what it did, and approvals on a phone.
- **Done when:** a team runs its agents through it for a month and keeps it.

## Before anything is published

- Choose the public name (decision 0007); check registries, domains, trademarks.
- Secure the domain used in schema IDs (decision 0006).
- Make the repository public; announce the spec as a draft, inviting schema proposals.
