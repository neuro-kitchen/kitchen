# kitchen-services

What a neuro-kitchen app's frame shows, without the frame: background jobs (progress, cancel),
the activity log (commands, focus, jobs, what was produced; in memory, never written), people
(who is working, so records say who did what), script environments (which Python, which packages),
and what the Changes panel compares (baselines, normalised JSON, the last commit).

kitchen-shell draws them; an agent, a test or a server reads the same state here. Moved out of
kitchen-shell in phase 2, step 3c ([runbook](../../docs/plans/0.0.10-standard.md)).
