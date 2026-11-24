# kitchen-code

The Code workspace the neuro-kitchen apps share: Python and Rust scripts that become commands
(menus, search, an agent's tools) and pipeline steps with typed ports.

```text
# kitchen: command = Count cells
# kitchen: hint = Counts the cells of the shown image
# kitchen: args = {"type": "object", "properties": {"min_area": {"type": "number"}}}
# kitchen: node = in image:image, mask:mask; out counts:table
```

A script reads `{"args": …, "context": …}` on its stdin (the context is what the app shows) and
prints its result as JSON on its last line. Its errors are read back onto its own lines.

`cargo run -p kitchen-code --example code-gallery` opens the workspace in the shared frame.

It was kitchen-shell's Scripts workspace (moved here whole, renamed `CodeWorkspace`) and lucem's
Code workspace beside it.
