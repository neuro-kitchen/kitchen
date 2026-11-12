# kitchen-shell

The frame the neuro-kitchen apps ([dsp-kitchen](https://github.com/neuro-kitchen/dsp-kitchen),
lumen) share, on [GPUI](https://www.gpui.rs) through gpui-kit and the
[kitchen-ui](https://crates.io/crates/kitchen-ui) design tokens and widgets.

- **One command registry.** An app declares each command once (id, title, icon, what it does,
  group, key, when it can run and why not, the JSON Schema of its arguments). Menus, key bindings,
  the search (Ctrl+K) and an agent's tools all come from that list, so none drifts. Menu entries
  can follow the app's state (`MenuItem::Run` with arguments, `Registry::provide`).
- **The frame** (`Shell`): the title bar with the app menu, the workspace switcher and where the
  app runs; each workspace's menu row; a status bar with the running jobs and CPU / memory;
  Settings with a built-in Appearance section; the search; an overlay; the window frame.
- **Jobs**, the **view container**, the **Changes panel** and a **Scripts workspace**.

`cargo run -p kitchen-shell --example shell-gallery` in the
[repository](https://github.com/neuro-kitchen/kitchen) shows each piece.

An app using kitchen-shell takes kitchen-ui from the same source (both from crates.io, or both from
the same git tag): two copies of kitchen-ui would be two `Theme` globals.

Licensed under MIT or Apache-2.0, at your option.
