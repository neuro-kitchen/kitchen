# kitchen

Shared code for the neuro-kitchen apps ([dsp-kitchen](https://github.com/neuro-kitchen/dsp-kitchen),
lucem, garnish, neuro-convert and every app made from
[kitchen-template](https://github.com/neuro-kitchen/kitchen-template)): one shell for every app,
which registers what it can do (commands, workspaces, settings) and gets the same menus, search,
panels and look; and the backend pieces the apps share (pipeline, compute, network, downloads).

The pipeline contract these crates are moving toward, Strata (a working name), is written down in
[`standard/`](standard/README.md): the spec, its decisions, examples and the
[roadmap](standard/roadmap.md). Kitchen follows it from 0.0.10. `docs/plans/` holds the plans in
progress.

Status: early work in progress. `kitchen-ui`, `kitchen-views` and `kitchen-shell` are on
crates.io; the other crates come from git.

## Using it from an app

`kitchen-ui`, `kitchen-views` and `kitchen-shell` are on crates.io (`kitchen-shell = "0.0.9"`).
Take every kitchen crate from the same source: all from crates.io, or all from the same git tag.
Two copies of `kitchen-ui` would be two `Theme` globals. The other crates come from the tag:

```toml
[dependencies]
kitchen-ui = { git = "https://github.com/neuro-kitchen/kitchen", tag = "v0.0.9" }
kitchen-shell = { git = "https://github.com/neuro-kitchen/kitchen", tag = "v0.0.9" }
kitchen-pipeline = { git = "https://github.com/neuro-kitchen/kitchen", tag = "v0.0.9" }
```

The repository is public: fetching it needs no token, locally or in an app's CI.

To work on an app and kitchen together, point the app at a local checkout (on that machine only,
in a git-ignored `.cargo/config.toml`, or commented in and out of the app's `Cargo.toml`):

```toml
[patch."https://github.com/neuro-kitchen/kitchen"]
kitchen-ui = { path = "../kitchen/interface/kitchen-ui" }
kitchen-shell = { path = "../kitchen/interface/kitchen-shell" }
```

## The crates

In three layers ([architecture](standard/architecture.md)); dependencies point down only, checked
by `cargo xtask check-layers` in CI against [`kitchen.toml`](kitchen.toml).

**`contract/`**: the [standard](standard/README.md) as Rust types, depending on nothing of ours.

| Crate | What it holds | Try it |
|---|---|---|
| `kitchen-contract` | Types (shared and `app:name`) and formats; op descriptions (version from 1, ports, parameters, margin, source, `finish`, `effects`, availability); the catalog; the pipeline format and its check; content ids, run records, run keys. The JSON Schemas in `standard/schemas/` are generated from it, and the standard's examples are its test fixtures | its tests |

**`runtime/`**: one back end for every app.

| Crate | What it holds | Try it |
|---|---|---|
| `kitchen-engine` | Runs a checked pipeline over the data an app walks (tiles, windows), piece by piece: the `Domain` an app provides, `Operation`, progress, cancel, preview | its tests |
| `kitchen-scripts` | Script ops: decorated Python functions and Rust files read as ops, checked in their own process, run by warm workers; the Python decorators | its tests |
| `kitchen-artifacts` | What the tools read and write, by kind, as the orchestrator sees it | its tests |
| `kitchen-compute` | The CubeCL layer every engine reaches a runtime through: runtime choice, device lifetime, launch geometry, autotune | its tests; GPU tests in [`notebooks/gpu-tests.ipynb`](notebooks/gpu-tests.ipynb) (Colab) |
| `kitchen-network` | A server's certificate authority, pairing with a short code, TLS for QUIC | its tests |
| `kitchen-downloads` | Verified downloads (SHA-256 and size) and the model cache | its tests |
| `kitchen-plugin` | Plugins as separate programs: the JSON-lines protocol, the handshake, discovery, a client, the manager and its Settings page | `cargo run -p kitchen-plugin --example plugin-gallery` |
| `kitchen-tool` | How the orchestrator drives a tool without its window, on kitchen-plugin | its tests |

**`interface/`**: what the apps draw with; it reaches the runtime only through its client (from
phase 2, step 4).

| Crate | What it holds | Try it |
|---|---|---|
| `kitchen-ui` | Theme tokens (neutrals, accents, selection, contrast), widgets, the parameter editor, code, diff and text views, the chat pane, icon bundling and its test | `cargo run -p kitchen-ui --example gallery` |
| `kitchen-views` | Views the apps share: the dock (tabs dragged into splits), one time cursor per workspace, video with exact frame times, keypoints | its tests |
| `kitchen-shell` | One command registry for menus, search, keys and agents; the frame (title bar, workspaces, menu row, status bar with jobs, Settings); the file picker; the Changes panel | `cargo run -p kitchen-shell --example shell-gallery`, `--example frame` |
| `kitchen-editor` | The Code workspace: scripts that become commands or pipeline steps, change marks, an external editor, language servers | `cargo run -p kitchen-editor --example code-gallery` |
| `kitchen-chat` | The agent workspace (sessions, conversation, Changes and Files) and the MCP host serving the registered commands | `cargo run -p kitchen-chat --example agent-gallery` |

An app registers what it can do as commands (`kitchen_shell::CommandSpec`), and gets from that
one list its menus, the search (Ctrl+K), its keys and an agent's tools. With the agent gallery
running, an MCP client connects through `kitchen-mcp-bridge <socket>` (the socket's path is in
Settings ▸ Agents); what an agent may run without asking is a setting.

## Building

`kitchen-compute`'s CPU runtime downloads an LLVM bundle on its first build. A machine without a
GPU builds and tests everything else; the CUDA and wgpu tests run on Colab (see the notebook
above). `scripts/check-all.sh` builds and tests the workspace the way the release does.

## Releasing

Version branches and pull requests into `main`, as dsp-kitchen: the workspace version and a
`## [x.y.z] - date` section of `CHANGELOG.md` are set in the release pull request; after the merge,
tag the merge commit `vx.y.z` and push the tag. `.github/workflows/release.yml` checks the tag
against the version, packages the crates, publishes the `publish = true` ones to crates.io by
trusted publishing (environment `crates-io`, approved by a reviewer) and makes the GitHub release
from the changelog section.

Licence: MIT OR Apache-2.0 ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)).
