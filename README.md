# kitchen

Shared code for the neuro-kitchen apps ([dsp-kitchen](https://github.com/neuro-kitchen/dsp-kitchen)
and the imaging app): one shell for every app, which registers what it can do (commands,
workspaces, settings) and gets the same menus, search, panels and look.

Status: early work in progress. Nothing here is published; the apps do not depend on it yet.

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

Planned crates:

- `kitchen-ui`: theme tokens, widgets, the parameter editor, icon bundling and its test.
- `kitchen-shell`: the frame (title bar, menu row, status bar with jobs), the search, the view
  container, side panels, the Settings shell.
- `kitchen-hub`: verified downloads and the model cache.
- `kitchen-plugin`: the plugin protocol (JSON lines, version handshake, capabilities) and a
  plugin manager (install, update, remove, disk use).
- `kitchen-agent`: the agent workspace shared by the apps (chat pane, sessions, remote
  workspaces, the MCP host). Each app registers its own capabilities: its document commands,
  analysis steps and queries.

Licence: MIT OR Apache-2.0 ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)).
