# Security policy

## Reporting a vulnerability

Please report it privately, not in a public issue: open a
[security advisory](https://github.com/neuro-kitchen/kitchen/security/advisories/new) on this
repository (once private vulnerability reporting is available for it), or contact the
maintainer, jesusdpa1, through GitHub. Include what is affected (which crate), the version or
commit, and how to reproduce it.

You will get an answer within a week. Fixes are described in the advisory and the
[CHANGELOG](CHANGELOG.md).

## Supported versions

kitchen is in early development (0.0.x) and not released: only the latest commit on `main`
receives fixes.

## Scope

Of particular interest, as the crates arrive: downloads through `kitchen-hub` (each verified by
SHA-256 and size), the plugin protocol and plugin installs of `kitchen-plugin` (programs run by
the apps), and the agent tools served by `kitchen-agent`.
