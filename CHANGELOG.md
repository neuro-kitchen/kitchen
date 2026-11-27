# Changelog

All notable changes to kitchen are written here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/) once releases start.

## [Unreleased]

### Changed

- **Three layers** (phase 2, step 1: moves only): the crates live in `runtime/` and `interface/`,
  with `contract/` for the contract crate to come; `cargo xtask check-layers` checks in CI that
  dependencies point down only (`kitchen.toml`), with one known exception (`kitchen-plugin`'s
  Settings page on `kitchen-ui`).
- **Renamed** (not on crates.io): `kitchen-net` → `kitchen-network`, `kitchen-hub` →
  `kitchen-downloads`, `kitchen-code` → `kitchen-editor`, `kitchen-agent` → `kitchen-chat`. The
  editor's action namespace (`kitchen_code::…`, used by key bindings) is unchanged.
- **The standard** moved into `standard/` (decisions 0014–0016).

### Added

- **The contract crate** (`kitchen-contract`, phase 2, step 2): the standard as Rust types.
  `Type` (shared names and `app:name`) and `Format`, merging kitchen-pipeline's port types and
  kitchen-artifacts' kinds (`types::from_kind` reads the old kinds); `OpSpec` with a version from 1,
  `Margin` (was `Context`), `finish`, `effects` and availability, and the `Recipe` source; the
  catalog with `fillable_by`; `Pipeline` (was `Graph`; `kitchen_pipeline` still reads as `version`)
  and its check, which names a chain when it meets another app's op; `ContentId`, canonical JSON,
  `ResultRecord`, `RunRecord`, `RunKey`. JSON Schemas generated into `standard/schemas/`, the
  standard's examples as test fixtures. kitchen-pipeline and kitchen-artifacts use it: their old
  names (`PortType`, `Context`, `Graph`, `ArtifactId`) are aliases now, not definitions. Tool run
  keys are unchanged.

## [0.0.9] - 2026-11-26

Every crate the apps share is in kitchen now, built and tested together, on the latest libraries.
Apps take them from the tag `v0.0.9`; `kitchen-ui`, `kitchen-views` and `kitchen-shell` are also
on crates.io.

### Added

- **The compute layer** (`kitchen-compute`): run-time choice of a CubeCL runtime
  (`ComputeTarget`), device lifetime (`configure_runtime`, `shutdown_devices`, `exit_now`),
  launch geometry with its position helpers, autotune helpers, pinned choices for reproducible
  runs, device-synchronised timing, and the 32-bit buffer limit. Merged from dsp-kitchen's
  `dspk-core` and lucem's `lucem-core`; environment variables keep each project's names through
  `set_env_prefix`. Tested on the CPU runtime; the CUDA and wgpu tests run on Colab
  (`notebooks/gpu-tests.ipynb`).
- **The network layer** (`kitchen-net`): a server's own certificate authority and client registry
  (certificates and tokens, revocation), TLS for QUIC (mutual TLS, pairing connections, a
  project's own protocols), and pairing with a short code over SPAKE2 (the server's side, a
  computer's ending in a certificate, a browser's ending in a token). Merged from dsp-kitchen's
  and lucem's stream crates; each project keeps its names on the wire through `set_service`.
- **The pipeline** (`kitchen-pipeline`): operations as data (`OpSpec`), the catalog an app offers,
  graphs with typed ports, the engine that runs a graph piece by piece over the data an app walks
  (`Domain`), and scripts in Python and Rust that declare themselves with decorators, are checked,
  and run only once confirmed.
- **Views the apps share** (`kitchen-views`): the dock (moved from kitchen-shell, which
  re-exports it), one time cursor per workspace, clocks aligned by sync pulses, video from image
  sequences or MP4 (H.264 decoded in Rust, feature `h264`) with keypoints over it.
- **The Code workspace** (`kitchen-code`): scripts that become commands or pipeline steps, with
  header lines, change marks, an external editor and completions from a language server. Scripts
  moved here from kitchen-shell.
- **Artifacts** (`kitchen-artifacts`): what the tools read and write as the orchestrator sees it:
  kinds (`zarr/nwb`, `table/parquet`, `raw/*`), content identity of files and Zarr stores,
  provenance records written next to each artifact, and run keys so a finished run is reused.
- **Headless tools** (`kitchen-tool`): how the orchestrator drives a tool without its window:
  `describe`, `validate` and `run` over the plugin protocol; `HeadlessTool` and `serve_stdio` for
  a tool, `ToolClient` for the orchestrator.
- **Plugin servers from any command line** (`PluginProcess::start_command`): `dspk tool serve`, or
  a tool started through `podman run`.
- **The Design System's tokens** (`kitchen-ui`): neutrals with a slight cool bias; `canvas`,
  `grid` and `info`; the selection (accent up to 16% with a 1 px edge, lower only where muted
  text would fall under 4.5:1) and the focus ring; the Okabe–Ito data colours; the palettes
  `TEAL` (lucem), `PAPRIKA`, `AMBER` and `COBALT`; IBM Plex Sans and Mono when an app ships them;
  the standard icons (views, Pipeline, Code, the file picker's places). Contrast tests cover
  every colour pair in both themes and every accent.
- **The frame** (`kitchen-shell`): the title bar in the order File · Edit · View · Help, the app's
  views, then search, Pipeline and Code and the theme; Open… (Ctrl/⌘+O) for apps that say what
  they open; narrow windows fold views to icons (under 1100 px), side panes into the Panels
  drawer (under 900 px) and the menus into one button (under 560 px); keys Ctrl/⌘+1…9, +Alt+P,
  +Alt+C, +\, +Q. `examples/frame` is a stand-in dsp-kitchen.
- **The file picker** (`kitchen-shell`): a folder model with unit tests, and its popup, with
  accessible roles on rows and places.
- **CI** (`.github/workflows/ci.yml`): stages A–C of `scripts/check-all.sh` on every pull request.
- **GPU tests on Colab** (`notebooks/gpu-tests.ipynb`).

### Changed

- **Latest libraries:** gpui-kit 0.11, jiff 1.0, toml_edit 0.25.21, rmcp 3.12,
  agent-client-protocol 5.6, sysinfo 0.40, sha2 0.11.2 and the rest of the lock file.
  `getrandom` stays at 0.2.
- `kitchen-views` is published with `kitchen-shell`, which depends on it.
- `EOSIN` is a deprecated alias of `TEAL`.

## [0.0.8] - 2026-11-18

### Added

- **Key bindings the user edits** (`kitchen_shell::keys`): `keybindings.toml` in the app's
  configuration folder, a key or a list of keys per command, in sections (`[commands]` for the
  registry's, one per part of an app with its own keys). `load_keybindings`, `resolve` (a
  section's keys against its defaults), `bind_commands` (the registry's commands, from
  `[commands]`), `write_default_keybindings` (every default, when the file is missing),
  `take_notices` (what could not be read: the default is kept), `parse_key`. Moved from
  dsp-kitchen, so lucem and dsp-kitchen read the same file format.

### Fixed

- **A click on an overlay no longer reaches what is under it** (`Shell::set_overlay`): the
  overlay sits on an occluding layer.

## [0.0.7] - 2026-11-17

### Added

- **People** (`kitchen_shell::people`): who works in the app: people (a name, optionally an
  affiliation and an ORCID iD, checked), the one working now, or a guest (recorded as `guest`,
  no name). Kept in a JSON file the app names (`people::load`); no passwords. With
  `ShellSpec::people`, the title bar shows a person button (pick someone, a guest, People…) and
  Settings ends with a People section (add, work as, forget). Commands `people.use`,
  `people.manage`.
- **What a session made**: each activity names who it was done for (`Activity::by`: the person
  working, or `guest`); `activity::view_of(title, keep, cx)` is a timeline of the activities a
  filter keeps, and `activity::made` keeps what was made (jobs, activities that produced files)
  rather than every command. The log stays in memory for the app's session, written nowhere.

### Changed

- `Activity` has a new field, `by` (struct literals add it or `..Default::default()`).

## [0.0.6] - 2026-11-17

### Added

- **`scrub_field`**: a number shown as `‹ value ›` that steps when dragged left or right (as
  `scrub_label`) and, pressed and let go without moving, calls `on_click` so the app swaps in its
  own text input to type the value. In the gallery beside the scrub label.

### Fixed

- **The title bar fits narrow windows** (a phone, a browser made small): below 1100 px the
  project name is left to the app's status bar and the remote shows as an icon; below 640 px the
  workspaces show only their icons and the layout toggles and the theme move into a "More" menu.
  Before, the controls on the right were pushed out of the window and could not be reached.
  `TitleFit` (and `TITLE_COMPACT_W`, `TITLE_NARROW_W`) names the three widths.

## [0.0.5] - 2026-11-16

### Changed

- Dependencies updated: gpui-kit 0.10.0 (GPUI snapshot 0.3.12), jiff 1.0 (`Zoned` is now
  `ZonedDateTime`), agent-client-protocol 5.3, rmcp 3.10, ureq, sysinfo and the lock file.

### Fixed

- `kitchen-ui` and `kitchen-shell` build for the browser (`wasm32-unknown-unknown`): no tree-sitter
  grammars there (the code view is plain text), gpui-kit's web asset source, `web-time` for the
  activity clock. A browser build needs nightly Rust: gpui's web platform depends on
  `wasm_thread`, which uses the unstable `stdarch_wasm_atomic_wait`.
- In a browser the component icons are embedded (gpui-kit's downloader failed on every frame and
  logged each time); an app can also give the address they are served from
  (`AppAssets::served_from`).

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
