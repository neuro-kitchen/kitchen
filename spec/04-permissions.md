# 4. Permissions

*Draft.*

A store decides every read and write against **grants**. Grants are records too
(`strata.dev/core/grant@1`, see [§6](06-schemas.md)), so who may do what has a history like
anything else.

## 4.1 A grant

| Field | Meaning |
|---|---|
| `to` | The actor it is for ([§2.3](02-identity.md)): a person, an app, an agent. |
| `from` | Who gave it (must hold at least what they give). |
| `can` | Any of `read`, `write`, `retract`, `grant`. |
| `on` | What it covers: schema patterns (`neuro-kitchen.org/ledger/*`), specific records, or records matching a simple filter on body fields (`{"account": "rec_01JD…"}`). |
| `approve` | Which of its actions wait for a person's approval: `none`, `writes`, `retracts`, or a filter (`{"amount.value": {"lt": "-500"}}`). |
| `until` | Optional end (RFC 3339). |
| `note` | Why, in words. |

A store's owner holds every right; everyone else starts with none.

## 4.2 Approvals

When an action needs approval, the store does not apply it. It records a **pending change**
(`strata.dev/core/pending@1`): the change as proposed, who proposed it, through what, and which
grant asked for approval. A person allowed to approve sees it (in any app, or a notification)
and answers. Approving applies the change, with a link to the approval; refusing records the
refusal. The agent that proposed it can `subscribe` to learn the outcome.

This is the same flow kitchen uses for scripts (checked, then confirmed by a person), and the one
the agent gateway pilot is about: an agent can do a lot, and the actions that matter wait for a
yes.

## 4.3 Delegation

An actor holding `grant` may give part of what it holds to another (a person to their agent, an
app to a plugin), never more. Revoking a grant revokes what was delegated from it.

## 4.4 Open questions

- Filters in grants (`on`, `approve`): how small can the filter language stay? Equality and
  numeric comparison on body fields may be enough; anything more becomes a query language.
- Read grants on parts of a record (hide `amount` from a viewer)? Probably not in the core:
  share a view record instead.
