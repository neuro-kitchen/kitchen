# 0005. Agents are actors with grants; actions that matter wait for approval

**Context.** Much of the reading and writing will be done by AI agents acting for people: generating
interfaces, recategorizing transactions, filing results. Giving an agent a person's full access is
what people do today, and it is the main risk of the whole idea.

**Decision.** People, apps and agents each have an identity. Every change records who is
responsible (`by`) and through what (`via`). Agents hold grants like anyone, narrowed to schemas,
records or fields' values, and a grant can require a person's approval for some or all of its
writes. Pending changes and approvals are records.

**Consequences.** "What did my agent do?" is a history query; "let it categorize but not spend" is a
grant. Approval flows need an interface to answer them (any app, a notification), which a store
must offer or delegate. This is the same check-then-confirm pattern neuro-kitchen uses for
scripts, and the product the agent-gateway pilot explores.

**Alternatives.** Leave authorization to stores: simpler spec, and every store would do it
differently, so agents could not be moved between them safely.
