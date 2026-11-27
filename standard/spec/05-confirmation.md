# 5. Checking, and confirmation (v0.2)

*Draft, from kitchen-pipeline's script review. Confirmation is deferred to the contract's v0.2
(decision 0016); until then a script op runs once it is checked.*

Code written by people and by AI will be added to pipelines constantly. Before a script op or a
recipe runs, the runtime **checks** it. A person **confirming** it is the second gate, planned for
v0.2.

## 5.1 States now

```text
  read ──problems──► Unreadable          (does not declare a usable op)
   │
   ▼
  Unchecked ──check──► Failed            (does not load or compile; with the line)
                │  └─► Mismatch          (runs differently from what its text declares)
                ▼
              Checked ──edited──► Unchecked (checked again, with what changed)
```

- **Check**: the runtime loads the script in a separate process, without data. It must load, and
  what its function declares (inputs, outputs, parameters) must equal what its text says.
- An edit checks it again and says what changed (a new parameter, a removed input).

## 5.2 What may run now

| | Preview (one piece, nothing stored) | Run (results stored) |
|---|---|---|
| Built-in and tool ops | yes | yes |
| Checked script op or recipe | yes | yes |
| Unchecked, failed, mismatched | no | no |

A run record of a script op names the hash that ran, so what ran is always known.

## 5.3 Agents now

An agent can write ops and pipelines and ask for them to be checked; what it runs is recorded `by`
the person it acts for, `via` the agent ([§3.2](03-results.md)). On a person's own machine this
is enough; a lab server shared by many people is where v0.2's confirmation matters.

## 5.4 Planned for v0.2: confirmation

- **Confirm**: a person sees the op's description (and, after an edit, what changed) and confirms
  it, tied to the content hash of the file or the recipe (which includes its scripts' hashes).
- **Whose confirmation counts**: each person's by default; a lab may trust confirmations by named
  people (its maintainers), and on the orchestrator roles decide.
- **What may run**: a checked op previews; only a confirmed one runs.
- **Records**: confirmations are records (who confirmed which hash, when), appended like run
  records; a run names the confirmation it ran under.
- **Agents** may ask for checks but never confirm.
