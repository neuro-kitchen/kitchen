# kitchen-code

The Code workspace the neuro-kitchen apps share. Code is how a user takes an app further than it
was written: a garnish statistic, a dspk operation, a lucem filter, each a Python or Rust script
that becomes a **command** (menus, search, an agent's tools) or a **pipeline step** with typed
ports.

```text
# kitchen: command = Count cells
# kitchen: hint = Counts the cells of the shown image
# kitchen: args = {"type": "object", "properties": {"min_area": {"type": "number"}}}
# kitchen: node = in image:image, mask:mask; out counts:table
```

A script reads `{"args": …, "context": …}` on its stdin (the context is what the app shows) and
prints its result as JSON on its last line. Its errors are read back onto its own lines.

## In the workspace

| | |
|---|---|
| **Change marks** | changed lines marked in the gutter, git style (added, modified, removed), against the last save, the opening, or the last commit when the script is in a repository. A toolbar toggle with the count (`+3 ~1 −2`). |
| **External editor** | "Open in editor" starts VS Code, Zed, PyCharm… (the app's setting, `KITCHEN_EDITOR`, `VISUAL`, `EDITOR`, a known editor on the `PATH`, or the system's). The workspace follows the file: written there, it is read again here; edited in both, it asks which to keep. |
| **Completions** | the app's names (`CodeWorkspace::set_vocabulary`), Python's, and the script's own; hover help. No language server needed. |
| **Add to pipeline** | a node script goes into the app's open pipeline (`CodeEvent::AddToPipeline`). |

## Keys

The workspace binds its own keys, in the `[code]` section of `keybindings.toml`, only while it has
focus (`keys::bind_keys` at start):

| Command | Default |
|---|---|
| `toggle_changes` | Ctrl/⌘ + Alt + G |
| `open_external` | Ctrl/⌘ + Shift + E |
| `run` | Ctrl/⌘ + Enter |
| `save` | Ctrl/⌘ + S |

`cargo run -p kitchen-code --example code-gallery` opens the workspace in the shared frame.

## Where it came from

kitchen-shell's Scripts workspace (moved here whole, renamed `CodeWorkspace`) and lucem's Code
workspace (its completions, "Add to pipeline"). lucem moves onto this crate next: its vocabulary
(`lucem.*`, NumPy, `scipy.ndimage`) through `set_vocabulary`, its `# lucem:` headers as `kitchen:`.

**Written, not built**: the session that wrote it had no build step.
