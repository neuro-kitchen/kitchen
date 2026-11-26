# 0013. Figures are our own grammar of graphics, in Rust

**Context.** The `figure` type needs an interactive form every app can draw: dsp-kitchen's rasters,
lucem's measurement plots, garnish's publication figures, a step's output a scientist asks Claude
for. The obvious candidate was Vega-Lite, a JSON grammar of graphics. But it is drawn by a JavaScript
library, and neuro-kitchen draws everything in Rust (GPUI on the desktop, the same code in the
browser through WebAssembly). Embedding a JavaScript runtime to draw plots would add exactly the
dependency the apps avoid.

**Decision.** `plot-json` is our own format: a grammar of graphics in the manner of ggplot2,
written as data and drawn by Rust code every app shares.

- **The grammar** (ggplot2's layers): data (a `table` result, by content ID), aesthetic mappings
  (`x`, `y`, `colour`, `fill`, `size`, `shape`, `group`), geoms (point, line, bar, area, histogram,
  boxplot, violin, raster, ribbon, text, rule), statistics (identity, bin, count, density,
  summary, smooth), scales (linear, log, sqrt, time, categorical; colour from the Design System's
  data colours), coordinates (cartesian, flipped, polar), facets (wrap, grid), and a theme from
  the design tokens.
- **One drawing for two uses**: the same description is drawn interactively in the apps (hover,
  zoom, linked selection) and rendered static (SVG, PNG, PDF) for papers, with the same layout.
- **Its own module of the core**, implemented once; garnish's figures are the first user (its
  `garnish-plot` moves onto it).
- A figure result stores the description and points at its data by content ID, so its lineage
  reaches the raw recording like any other result.

**Consequences.** No JavaScript anywhere; figures look the same in every app and in print; a
figure an agent writes is data it can read back and change. We own a plotting library: a real
amount of work, done once, with ggplot2 as the reference for what to include and in what order.
Exporting to Vega-Lite or matplotlib for people who want them stays possible as a step.

**Alternatives.** Vega-Lite (JavaScript to draw); Plotters, a Rust plotting crate (drawing
primitives, not a grammar; usable underneath for static output); a fixed set of plot kinds (what
every app does today, and why each draws its own).
