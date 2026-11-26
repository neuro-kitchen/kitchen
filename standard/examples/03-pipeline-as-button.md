# 03. A pipeline becomes a button in lucem's Explore

*Carla built a pipeline with Claude: denoise → threshold → watershed → measure, giving cell labels
and a measurements table. She wants a button in Explore that runs it on whatever she is looking at.*

## Walk-through

1. **The pipeline is data**: sources `image: image`; outputs `cells: labels`, `measurements: table`;
   built-in steps plus one confirmed script ([spec §4.1](../spec/04-pipelines.md)).
2. **Save it as a step** (`pipeline.save_as_step`): Carla exposes two parameters (`threshold`,
   `min_area`), the rest fixed. Its version is the pipeline's hash, which includes the script's
   hash. From outside it is one step: `carla:cell_detect` ([spec §2.3](../spec/02-steps.md)).
3. **Confirm it**: one confirmation covers the composite and what is in it, because it is tied to
   the hash that includes everything. Editing any part asks again.
4. **Pin it** (`offers` / `pin`): Explore's view-model offers *current image* (`image`) and
   *selected region* (`region`). The step's input `image` is filled by *current image*. Carla pins
   it; it becomes a command (name, icon, optional key) in Explore's menu row or tool panel, and,
   through the command registry, in the search and the agent's tools.
5. **Press it**: the view-model calls `run` with the current image; lucem's domain walks the slide in
   tiles; results are stored with a run record pointing at the composite's hash.
6. **Shown by type**: `labels` as an overlay, `table` in the side panel. Explore does not need to
   know this step: it knows how to show those types.
7. **Shared**: a labmate adds the composite's file to the lab's folder and confirms it; where the
   button sits is each person's choice.

## Requires

- Composite steps ([spec §2.3](../spec/02-steps.md)).
- Offers and pins ([spec §4.5](../spec/04-pipelines.md)).
- Each app's display-by-type: overlays, tables, figures (core `interface/views`, app view-models).
