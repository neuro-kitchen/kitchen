# 0011. The migration order: young projects first

**Context.** Moving every project onto the contract at once would freeze dsp-kitchen and lucem for
months, and design the contract around whichever app was moved first.

**Decision.** Build and merge the existing kitchen work as it is; write the standard; restructure
the core (move, then extract the back end from the interface crates, then merge the contract);
move garnish and the orchestrator (young, few users); bring neuro-convert (an existing tool, the
rehearsal); then dsp-kitchen, then lucem; update the template before new projects are generated.
dsp-kitchen's and lucem's needs are written into the contract from the start (example 04), so the
young projects do not shape it alone.

**Consequences.** Each step ships on its own; dsp-kitchen and lucem keep shipping science meanwhile
(only their back-end parts about to be replaced are frozen).
