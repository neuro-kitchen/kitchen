# Changelog

All notable changes to kitchen are written here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/) once releases start.

## [Unreleased]

## [0.0.4] - 2026-11-13

### Added

- Jobs leave an outcome (`JobHandle::finish`: what they found, or why they failed), kept after they
  end; a command that starts a job returns `{"job": id}` (`jobs::job_reply`). The MCP host then
  answers the agent when the job ends, with that outcome, instead of at once with `null`.
- A command can return an image (`{"image": {"data": base64, "mime_type": …}}`): the MCP host
  sends it as image content, so an agent can look at it.
- `kitchen-shell` is published on crates.io too (0.0.3 published by hand on 2026-11-12, from the
  v0.0.3 tag with this packaging: README, licence texts, keywords); the release workflow
  publishes it from now on (it needs its trusted publisher on crates.io: this repository,
  `release.yml`, environment `crates-io`). Apps that publish their own crates take kitchen-ui and
  kitchen-shell from crates.io.
- Agents see each command under a tool name MCP clients accept (`hist.find_cells` →
  `hist_find_cells`; `tool_name`); a call by either name reaches the command. Antigravity refused
  every dotted name, which left the chat's agent without lumen's tools.
- `AgentWorkspace::set_preamble`: what the agent is told before the first prompt of each session
  (where it works, what is open, which tools act in the app).
- `kitchen-ui` timeline (`timeline`): an episodic memory of the work (after Scott Jenson's
  prototype): what the person and the agent did, by lane, as bars coloured by kind, as wide as
  they lasted and as tall as they were busy, on an hour axis; a card on hover; a click shows the
  episode's context cards: each action, where it came from and what it produced. Many actions
  are summarised into a few episodes (`episodes`: same-kind neighbours merge, the shortest fold
  into their neighbours), or shown one by one. A gallery page and a headless test.
- `kitchen-shell` activity log (`activity`): every command run (who: the person or an agent; how:
  menu, key, search, title bar, agent; its arguments in short and its result), the time in each
  workspace, every job from start to end, and what a step produced; `activity::view` is a
  timeline that follows it. `Registry::run_as` records; the frame, the jobs and the MCP host feed
  it by themselves.
- `kitchen-agent` ACP client (`acp`): the chat's agent is any program speaking the Agent Client
  Protocol (`agy acp`, Claude Code's adapter), started by the app with its MCP servers; prompts,
  cancel, new sessions; streamed answers, its own tool calls and its permission questions come
  back as `AcpUpdate`s. The agent keeps its own login: no key in the app.
- `AgentWorkspace::connect`: the conversation runs on a connected ACP agent; the agent's questions
  show above the prompt with its options; the model chip names it.
- `kitchen-agent` bridge as a library function (`bridge::bridge_stdio`), so an app's own program
  can be its MCP server for an agent (`kitchen-mcp-bridge` uses it).
- `kitchen-shell` project environments (`env`): the Python venv of a project's scripts (its own,
  the active one or the app's default; version and packages read without running it), made and
  synced by uv from `pyproject.toml` and `uv.lock`; the Rust workspace and `rust-toolchain.toml`.

## [0.0.3] - 2026-11-12

### Added

- `kitchen-shell` menus that follow the app: `MenuItem::Run` (a command run with arguments under
  its own title: an entry per image, per recent file, per model) and `MenuItem::Provided` (the
  entries a provider makes when the menu opens, `Registry::provide`); `Shell::set_menus` replaces
  a workspace's menu row; long menus scroll.
- `kitchen-shell` window frame (from lumen's and dsp-kitchen's): rounded corners while the window
  floats, square when maximised, full screen or tiled; it resizes from its own edge
  (`resize_edge`, `frame_rounded`); no shadow margin.
- `Shell::set_overlay`: one view of the app's over everything (a help sheet, a dialog);
  `Shell::workspace_id`.

### Changed

- README: an app using `kitchen-shell` (or any crate besides `kitchen-ui`) takes `kitchen-ui` from
  the same git tag, not crates.io: the shell depends on it by path inside this repository, and
  two copies would mean two `Theme` globals.

## [0.0.2] - 2026-11-12

### Changed

- Version 0.0.2 for every crate; the README says how an app depends on kitchen-ui from
  crates.io and the other crates from the tag, and how a release is made.
- CodeQL runs through GitHub's default setup (enforced by the organisation since the repository
  went public); the repository's own `codeql.yml`, which clashed with it, is removed.

### Added

- Release workflow (`release.yml`, as dsp-kitchen's): on a `v*` tag, the tag is checked against
  the workspace version and the changelog, the crates are packaged and checked by cargo-deny,
  the `publish = true` crates go to crates.io by trusted publishing (environment `crates-io`,
  approved by a reviewer), and the GitHub release takes this section as its notes.
- `kitchen-shell` crate. Command registry: an app declares each command once (`CommandSpec`:
  id, title, icon, hint, group, key, JSON Schema of its arguments, `enabled` with the reason
  when not, `checked` for toggles, `run`), and the `Registry` builds from that one list the menus
  (`MenuSpec`, submenus; a command that cannot run is greyed with its reason), the key bindings
  (`RunCommand`) and the search. Typed commands (`CommandSpec::with_args`) publish their schema
  and refuse arguments that do not read.
- `kitchen-shell` search (Ctrl+K): every command by group, title, hint or key; disabled ones
  listed greyed with the reason; ↑ ↓ Enter, Esc.
- `kitchen-shell` view container (from lumen's): a dock of views dragged into splits
  (`new_dock`, `add_beside`), `Arrangement` (tabs, side by side, stacked, grid) with `arrange`,
  the buttons in a view's tab (`tab_buttons`: its kinds and "+", 24 px targets), and the "Add view
  or container" dialog (`AddViewDialog`) over the kinds a workspace offers (`ViewOption`, greyed
  with the reason when unavailable).
- `kitchen-shell` frame (`Shell` from a `ShellSpec`): a title bar with the app's menu and Help,
  the workspace switcher (`WorkspaceSpec`), the project, the remote button (`RemoteSpec`), layout
  toggles (commands as icon buttons) and the theme; the current workspace's menu row; the
  workspace as a card on the chrome; a status bar with the status line, the running jobs (each
  with a cancel button) and CPU and memory meters; Settings (Ctrl+,) with a built-in Appearance
  section (theme, accent, text size) and the sections an app registers (`SettingsSection`); the
  search. The frame registers its own commands (`shell.search`, `shell.settings`, `shell.theme`,
  `shell.workspace.<id>` on Ctrl+1–9) and tells the app when the look changes (`ShellEvent`).
  A search over one kind of command (`open_search_with`: Process ▸ "Find an operation…"), and
  the app's own readings in the status bar (`ShellSpec::status_items`).
- `kitchen-shell` job registry (`jobs`, from lumen's): kind, name, progress and a cancel flag;
  one GPU job at a time; a job ends when its handle drops.
- `kitchen-hub` crate (lumen's and dsp-kitchen's hubs in one): an `Artifact` (id, family, file,
  URI on `hf://`, `gh://`, `zenodo://`, `https://` or `file://`, SHA-256, size) is fetched into
  the app's cache (`Hub::for_app`), hashed while it streams and committed only when hash and size
  match; progress and cancel; a cache index for `status` without hashing, `verify`, `remove`,
  `clean`, `disk_use`; `.tgz` archives unpacked once.
- `kitchen-plugin` crate (from lumen's model plugins): the JSON-lines protocol (`CoreRequest`,
  `Event`), manifests with the app's own fields (`PluginInfo`: protocol version, licence,
  capabilities, runtime status) and the handshake (`compatible`), discovery per app
  (`PluginKind`: prefix, folders, `$ENV`), a server client (`PluginProcess`), the `Manager`
  (install from a file or a verified download, update, remove with its downloads, disk use) and
  its Settings page (`ui::PluginsPage`); a gallery example over demo plugins.
- `kitchen-shell` Changes panel (`ChangesPanel`, on kitchen-ui's `DiffView`): watches the files
  that say what a project is (`session.json`, `pipeline.json`, `project.json`) and shows what
  changed as diffs compared by key, long arrays as one summary line; against the last save, the
  session's start or the last git commit (`Baseline`, refused with the reason outside a
  repository); a count of changed lines for a badge (`ChangesEvent::Changed`,
  `WorkspaceSpec::badge`); an agent's proposed change (`propose`) reviewed with Accept / Reject
  per hunk and written with only the accepted hunks (`apply_proposal`; JSON stays valid).
- `kitchen-shell` Scripts workspace (`ScriptsWorkspace`, on kitchen-ui's `CodeView`): a tree of
  Python and Rust scripts, the editor (errors marked on their lines: Python tracebacks, Rust
  compiler messages), a run panel (output, the JSON result, errors); runs off the UI thread as a
  job, with `{"args", "context"}` on the script's stdin (the context is what the app says is
  shown). A header (`# kitchen: command = …`, `hint`, `args` as JSON Schema, `node = in …; out …`)
  makes a script a command (`register`: in menus, search and an agent's tools) and a pipeline
  node with typed ports checked against the app's types (`nodes`); new scripts from a template.
- `kitchen-agent` crate. MCP host (`McpHost`, rmcp 3.9): the app's registered commands are an
  agent's tools (schemas and read-only hints from the registry), run in the app's window under a
  `Policy` (run anything; only read; or read freely and ask the person for the rest, allowed or
  denied in the workspace), every call recorded (`CallRecord`); served over any byte stream
  (`serve_io`), stdio, or a local socket (`McpHost::listen`) that MCP clients reach through the
  `kitchen-mcp-bridge` program. No API keys anywhere.
- `kitchen-agent` workspace (`AgentWorkspace`, on kitchen-ui's `ChatPane`): saved sessions
  (conversation and calls as JSON), the conversation with every tool call and the calls waiting
  for the person, the Changes panel or the project's files beside it; "Open in Agents"
  (`open_command`) and the Agents section of Settings (`settings_section`); a gallery with a
  scripted agent (no model).
- README: the five crates, what each holds and how to try it.
- Each crate's example has its own name (`shell-gallery`, `plugin-gallery`, `agent-gallery`;
  kitchen-ui keeps `gallery`), so a workspace build has no output name collision.
- `kitchen-ui` `CodeView`: gpui-kit's code editor (tree-sitter: Rust, Python and JSON), read-only
  or editable (`CodeChanged`), line numbers, current line, selection and copy; errors
  (`(line, message)`) get a red gutter mark, an underline with the message on hover, and a list
  under the code whose entries go to their line.
- `kitchen-ui` `DiffView` and the `diff` model (on `similar`): added and removed lines on green
  and red tints, old and new line numbers, hunk headers, unchanged runs folded behind
  "⋯ N unchanged lines"; a JSON mode comparing by key (sorted keys, long arrays as one summary
  line such as `cells: 2,539 → 2,712`); optional Accept / Reject per hunk (`HunkDecision`) and
  `Diff::apply` for the accepted hunks of a text.
- `kitchen-ui` `TextView`: wrapped, selectable light markdown with highlighted fenced code;
  text can stream in.
- `kitchen-ui` `chat::ChatPane`: user, agent and tool-call rows (a tool call opens to its JSON
  arguments and result), streamed agent text, a prompt box (Enter sends, Shift+Enter is a
  newline, Send becomes Stop while the agent runs), context chips and an empty state; it emits
  `ChatEvent::Send` / `Stop`, and `ChatMessage` is serializable for a session file.
- Theme: diff line tints (`diff_added`, `diff_removed`, checked for contrast in both themes), the
  code editor's colours and a monospace size two steps under the base text.
- Gallery pages for code, diffs and text, and for the chat; the headless test draws them in both
  themes, accepts and rejects hunks, and sends prompts from the chat pane.

## [0.0.1] - 2026-11-11

### Added

- Cargo workspace with the `kitchen-ui` crate (gpui-kit 0.9.1).
- The `kitchen-ui` package carries the MIT and Apache-2.0 licence texts.
- `kitchen-ui` design tokens: spacing on the 8 px grid, type scale (15 px base), radii by tier,
  pane gaps; shared neutrals (plain greys) and status colours for light and dark, checked for
  WCAG contrast; the accent `Palette` an app supplies (`LIME` for dsp-kitchen, `EOSIN` for the
  imaging app); `apply` hands them to gpui-kit's theme.
- `kitchen-ui` widgets: `Section`, `row`, `slider`, `MenuSelect`, `stepper`, `scrub_label`,
  `swatch`, `EmptyState`, `icon_button`.
- `kitchen-ui` parameter editor, generic over any op that serializes to JSON (`Params`).
- `kitchen-ui` icon bundling (`AppAssets`) and the check an app runs over its sources
  (`assert_icons_bundled`, aliased icon names included).
- Widget gallery example (`cargo run -p kitchen-ui --example gallery`) and a headless UI test.
- `kitchen-ui` accents: lumen's eosin rose (`Palette::EOSIN`) next to dsp-kitchen's lime; a user
  accent of any colour (`Look::accent`, an `Rgb` stored as `"#rrggbb"` in an app's preferences),
  turned into light and dark accents that keep the hue and pass WCAG contrast; the
  `AccentPicker` widget (the app's accent, suggested swatches, any colour), shown in the gallery.
- README: how an app depends on kitchen (git tag, local override, access while private).
- `kitchen-ui` is ready for crates.io: its own README, repository, keywords and category (not yet
  published).
- README: what kitchen is and the planned crates.
- `.gitignore`: local notes, data, model weights, secrets and build output stay out of git.
- Licences: `LICENSE-MIT` and `LICENSE-APACHE` (MIT OR Apache-2.0).
- `SECURITY.md`: how to report a vulnerability.
- Dependabot: weekly cargo and GitHub Actions updates, minor and patch grouped.
- Supply-chain checks: cargo-deny (`deny.toml`) on pushes, pull requests and weekly.
- CodeQL scanning of the Rust sources and workflows, run once the repository is public.
