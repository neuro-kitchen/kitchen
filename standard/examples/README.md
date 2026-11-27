# Examples

Each example is walked through against the contract and the runtime API, step by step, and ends
with what it requires. An example that cannot be walked through on paper is a gap in the spec; the
conformance suite turns each into tests.

| # | Example | Exercises |
|---|---|---|
| [01](01-missing-plot.md) | A plot is missing; Claude makes a reusable one | Script ops in the workspace, the `figure` type, checking, reuse, context inputs |
| [02](02-different-interface.md) | A person dislikes an interface; Claude makes another | The runtime API from outside an app, MVVM, embeddable viewers, `via` |
| [03](03-pipeline-as-button.md) | A pipeline becomes a button in lucem's Explore | Recipes, offers and pins, display by type |
| [04](04-hard-cases.md) | dsp-kitchen's windows, lucem's tiles, previews, curation | Pieces and margins, device data, huge inputs, manual ops. **Required**: the contract is not 1.0 until these pass |

[walkthrough.md](walkthrough.md) walks each example through the spec and lists what is left to
decide.
