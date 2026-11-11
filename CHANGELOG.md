# Changelog

All notable changes to kitchen are written here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/) once releases start.

## [Unreleased]

### Added

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
