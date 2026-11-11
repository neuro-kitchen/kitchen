# kitchen

Shared code for the neuro-kitchen apps ([dsp-kitchen](https://github.com/neuro-kitchen/dsp-kitchen)
and the imaging app): one shell for every app, which registers what it can do (commands,
workspaces, settings) and gets the same menus, search, panels and look.

Status: early work in progress. Nothing here is published; the apps do not depend on it yet.

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

Licence: MIT OR Apache-2.0.
