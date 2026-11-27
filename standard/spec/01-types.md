# 1. Types

*Draft, from kitchen-pipeline's `PortType` and kitchen-artifacts' `ArtifactKind`, which this part
merges.*

A **type** names what flows between ops and what is stored. Two ops connect when an output's
type matches an input's.

## 1.1 One type, two faces

Today kitchen has two systems: port types (`image`, `signal`: what flows inside a pipeline) and
artifact kinds (`raw/bin`, `zarr/ome`, `table/csv`: what is stored). They describe the same things
at two moments. The contract keeps one:

- **Type**: what it is (`image`, `signal`, `table`, `figure`). Ports and results both have one.
- **Format**: how a stored result is encoded (`ome-zarr`, `nwb`, `parquet`, `csv`, `svg`, `png`).
  Only stored results have one. A type lists the formats it can be stored in.

A port says `table`; the result it produced says `table` in `parquet`.

## 1.2 Names

- **Shared types** have bare names and are defined by the contract.
- **App types** are namespaced: `<namespace>:<name>`, the namespace being the app or project
  (`dspk:sorting`, `lucem:cells`, `convert:session`). An app defines its own without asking.
- Lowercase letters, digits, `-`; namespaces likewise.

## 1.3 The shared types

| Type | What it is | Typical formats |
|---|---|---|
| `signal` | Samples over time on channels: `[channels, samples]`, with a rate | raw (`bin`, `dat`), `zarr`, `nwb` |
| `events` | Points in time with labels (spikes, stimuli, TTLs) | `parquet`, `csv`, `nwb` |
| `image` | Pixels: a plane, a stack, a pyramid; channels | `ome-zarr`, `ome-tiff`, `png` |
| `mask` | Yes/no per pixel or sample | `ome-zarr`, `png` |
| `labels` | An integer per pixel or sample: objects | `ome-zarr` |
| `region` | Shapes in an image's or a recording's coordinates (polygons, intervals) | `geojson`, `json` |
| `table` | Rows and named columns | `parquet`, `csv` |
| `figure` | Something to look at: static, or a description an app draws interactively | `svg`, `png`, `pdf`, `plot-json` (our grammar of graphics, after ggplot2, drawn in Rust: [decision 0013](../decisions/0013-figures-are-a-grammar-in-rust.md)) |
| `video` | Frames with exact times | `mp4`, image sequences |
| `file` | Anything else, opaque | any |
| `any` | Accepts every type (inputs only) | |

New shared types follow principle 2: two apps need them.

## 1.4 Matching

- An output of type `T` feeds an input of type `T`, or of `any`.
- An input may accept several types (`signal | events`).
- No inheritance in this version: `dspk:sorting` does not "count as" `labels`. An op that wants to
  accept both says so. No "is-a" list in this version; revisit when two apps need the same subtype.

## 1.5 A type's description

Each type, shared or app, is described once: its name, one paragraph saying what it is, the
formats it can be stored in, and, for arrays, its axes (`[channels, samples]`, `[z, y, x]`) and the
kind of unit along each (time, length). The values (a recording's 30 kHz, a slide's 0.25 µm per
pixel) come from the data's own metadata and are stored with each result: an op reads them from
its input, never from the type. The description is what an agent reads to write an op that takes it correctly.

## 1.6 Open questions

- None left from the first draft (axes and units: decision 0016).
