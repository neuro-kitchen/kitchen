# kitchen

Shared code for the neuro-kitchen apps ([dsp-kitchen](https://github.com/neuro-kitchen/dsp-kitchen)
and the imaging app): one shell for every app, which registers what it can do (commands,
workspaces, settings) and gets the same menus, search, panels and look.

Status: early work in progress. `kitchen-ui` is on crates.io; the other crates are not published
yet, and the apps do not depend on them yet.

## Using it from an app

An app that only needs `kitchen-ui` can take it from crates.io (`kitchen-ui = "0.0.3"`). An app
that uses `kitchen-shell` (or any other crate here) takes every kitchen crate from the same git
tag, `kitchen-ui` too: the shell depends on `kitchen-ui` by path inside this repository, so a
`kitchen-ui` from crates.io beside it would be a second copy (a second `Theme` global).

```toml
[dependencies]
kitchen-ui = { git = "https://github.com/neuro-kitchen/kitchen", tag = "v0.0.3" }
kitchen-shell = { git = "https://github.com/neuro-kitchen/kitchen", tag = "v0.0.3" }
```

To work on an app and kitchen together, point the app at a local checkout (on that machine only,
in a git-ignored `.cargo/config.toml`, or commented in and out of the app's `Cargo.toml`):

```toml
[patch."https://github.com/neuro-kitchen/kitchen"]
kitchen-ui = { path = "../kitchen/crates/kitchen-ui" }
kitchen-shell = { path = "../kitchen/crates/kitchen-shell" }
```

## Releasing

Version branches and pull requests into `main`, as dsp-kitchen: the workspace version and a
`## [x.y.z] - date` section of `CHANGELOG.md` are set in the release pull request; after the merge,
tag the merge commit `vx.y.z` and push the tag. `.github/workflows/release.yml` checks the tag
against the version, packages the crates, publishes the `publish = true` ones to crates.io by
trusted publishing (environment `crates-io`, approved by a reviewer) and makes the GitHub release
from the changelog section.

While the repository is private, fetching it needs access: locally your GitHub login (for
example `git config --global credential.helper` with `gh auth setup-git`); in an app's CI a
read-only token or a deploy key for this repository.

## The crates

| Crate | What it holds | Try it |
|---|---|---|
| `kitchen-ui` | Theme tokens, widgets, the parameter editor, code, diff and text views, the chat pane, icon bundling and its test | `cargo run -p kitchen-ui --example gallery` |
| `kitchen-shell` | One command registry for menus, search, keys and agents; the view container; the frame (title bar, workspaces, menu row, status bar with jobs, Settings); the Changes panel; the Scripts workspace | `cargo run -p kitchen-shell --example shell-gallery` |
| `kitchen-hub` | Verified downloads (SHA-256 and size) and the model cache | its tests |
| `kitchen-plugin` | Plugins as separate programs: the JSON-lines protocol, the handshake, discovery, a client, the manager and its Settings page | `cargo run -p kitchen-plugin --example plugin-gallery` |
| `kitchen-agent` | The agent workspace (sessions, conversation, Changes and Files) and the MCP host serving the registered commands | `cargo run -p kitchen-agent --example agent-gallery` |

An app registers what it can do as commands (`kitchen_shell::CommandSpec`), and gets from that
one list its menus, the search (Ctrl+K), its keys and an agent's tools. With the agent gallery
running, an MCP client connects through `kitchen-mcp-bridge <socket>` (the socket's path is in
Settings ▸ Agents); what an agent may run without asking is a setting.

Licence: MIT OR Apache-2.0 ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)).
