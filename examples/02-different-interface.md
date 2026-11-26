# 02. A person dislikes an interface; Claude makes another

*Ben finds lucem's Analysis screen cluttered. He wants one slide, its cell counts per region, and a
button to re-run detection.*

## Level A: rearrange inside the app

Most dislikes are layout. The frame already lets Ben move and hide views, save arrangements, change
keys and theme. Claude can also generate a **workspace**: a view-model composing existing ones (the
slide viewer, a table, a pinned pipeline) and a view laying them out. It is saved as an arrangement
Ben switches to. Nothing about the back end changes.

## Level B: a new interface over the same runtime

1. **Ben asks Claude**: "Build me a simple page: slide, counts per region, a re-run button."
2. **Claude connects to the runtime** (`hello` over HTTP; the runtime runs beside lucem, or is the
   lab's orchestrator): `results` of type `table` and `labels` for this slide; `steps` finds the
   detection pipeline (a composite step).
3. **Claude generates the page**, a web page, which:
   - reads the counts (`result.read` on the table);
   - shows the slide through **lucem's own viewer, embedded** (tiles streamed by the runtime), not a
     rebuilt one;
   - calls `run` with the detection pipeline on this slide when the button is pressed, and
     `run.watch` for progress.
4. **The page is a caller like any other**: runs it starts are recorded `by` Ben, `via` the page.
   It can run confirmed steps; it cannot confirm anything.
5. **Results are shared**: what Ben runs from his page appears in lucem's Analysis, and in anyone
   else's interface, with its lineage.

## What makes this possible

- MVVM: lucem's own Analysis view-model reaches results and runs only through the runtime's client.
  Ben's page uses the same calls over HTTP. Nothing lucem's window can do is out of reach.

## Requires

- The runtime reachable outside the app: HTTP and MCP ([spec §6](../spec/06-runtime-api.md)).
- Heavy viewers (gigapixel slides, GPU traces) offered as embeddable components: a generated page
  composes them, it cannot reasonably rebuild them.
- `via` on every run ([spec §3.2](../spec/03-results.md)).
- Optionally, the design tokens published, so a generated page can look like neuro-kitchen.
