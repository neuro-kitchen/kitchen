# 5. Confirmation

*Draft, from kitchen-pipeline's script review.*

Code written by people and by AI will be added to pipelines constantly. A step from a script, a
composite step, or a pipeline runs only once it is **checked** and **confirmed**.

## 5.1 States

```text
  read ──problems──► Unreadable          (does not declare a usable step)
   │
   ▼
  Unchecked ──check──► Failed            (does not load or compile; with the line)
                │  └─► Mismatch          (runs differently from what its text declares)
                ▼
              Checked ──a person confirms──► Confirmed ──edited──► Checked (with what changed)
```

- **Check**: the runtime loads the script in a separate process, without data. It must load, and
  what its function declares (inputs, outputs, parameters) must equal what its text says.
- **Confirm**: a person sees the step's description (and, after an edit, what changed: new
  parameter, removed input…) and confirms it.

## 5.2 What a confirmation is tied to

By default, to the **content hash** of the file (or of the pipeline, which includes its scripts'
hashes): any edit asks again. A lab may choose to keep a confirmation while the step's description
is unchanged (an edit inside the function only), the check still running on every change.

## 5.3 What may run

| | Preview (one piece, nothing stored) | Run (results stored) |
|---|---|---|
| Built-in and tool steps | yes | yes |
| Checked script or composite | yes | no |
| Confirmed script or composite | yes | yes |

## 5.4 Confirmations are records

Who confirmed what (by hash), when, kept with the project and appended like run records. A run
record of a script step names the hash that ran, and the confirmation it ran under.

## 5.5 Agents

An agent can write steps and pipelines and ask for them to be checked; it cannot confirm them. An
agent acting for a person runs only what that person could, and the run record says so (`by` the
person, `via` the agent).
