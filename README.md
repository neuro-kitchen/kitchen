# kitchen

Shared code for the neuro-kitchen apps ([dsp-kitchen](https://github.com/neuro-kitchen/dsp-kitchen)
and the imaging app): one shell for every app, which registers what it can do (commands,
workspaces, settings) and gets the same menus, search, panels and look.

Status: early work in progress. `kitchen-ui` is on crates.io; the other crates are not published
yet, and the apps do not depend on them yet.

## Using it from an app

Apps depend on kitchen by git, at a tag:

```toml
[dependencies]
kitchen-ui = { git = "https://github.com/neuro-kitchen/kitchen", tag = "v0.0.1" }
```

To work on an app and kitchen together, point the app at a local checkout in a git-ignored
`.cargo/config.toml` in the app's repository (it overrides the git source on that machine only):

```toml
[patch."https://github.com/neuro-kitchen/kitchen"]
kitchen-ui = { path = "../kitchen/crates/kitchen-ui" }
```

While the repository is private, fetching it needs access: locally your GitHub login (for
example `git config --global credential.helper` with `gh auth setup-git`); in an app's CI a
read-only token or a deploy key for this repository.

## The crates

| Crate | What it holds | Try it |
|---|---|---|
| `kitchen-ui` | Theme tokens, widgets, the parameter editor, code, diff and text views, the chat pane, icon bundling and its test | `cargo run -p kitchen-ui --example gallery` |
| `kitchen-shell` | One command registry for menus, search, keys and agents; the view container; the frame (title bar, workspaces, menu row, status bar with jobs, Settings); the Changes panel; the Scripts workspace | `cargo run -p kitchen-shell --example gallery` |
| `kitchen-hub` | Verified downloads (SHA-256 and size) and the model cache | its tests |
| `kitchen-plugin` | Plugins as separate programs: the JSON-lines protocol, the handshake, discovery, a client, the manager and its Settings page | `cargo run -p kitchen-plugin --example gallery` |
| `kitchen-agent` | The agent workspace (sessions, conversation, Changes and Files) and the MCP host serving the registered commands | `cargo run -p kitchen-agent --example gallery` |

An app registers what it can do as commands (`kitchen_shell::CommandSpec`), and gets from that
one list its menus, the search (Ctrl+K), its keys and an agent's tools. With the agent gallery
running, an MCP client connects through `kitchen-mcp-bridge <socket>` (the socket's path is in
Settings ▸ Agents); what an agent may run without asking is a setting.

Licence: MIT OR Apache-2.0 ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)).
