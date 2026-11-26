# 04. The hard cases

garnish works on whole tables and the orchestrator and neuro-convert on whole files; none of them
stresses what makes dsp-kitchen and lucem hard. These cases are therefore written into the contract
from the start, and **the contract is not 1.0 until each runs on it**.

## A. dsp-kitchen: windows of a long recording, on the device

*A 3-hour, 384-channel recording at 30 kHz (≈ 250 GB as int16) goes through high-pass → common
reference → whitening → spike detection.*

- **Walk**: windows of time with the margin before and after each (the filters' padding). The
  recording is never loaded whole ([spec §4.3](../spec/04-pipelines.md)).
- **Device**: each window is uploaded once; every step runs on the GPU; the data between steps never
  leaves the device. The contract describes steps and pieces; how data flows between steps inside a
  piece is the domain's (its data type can be a device buffer).
- **Fitted steps**: whitening is fitted on a sample of the recording first, then applied per window.
  A fit is a step whose output (`dspk:whitening`) is an input of the apply step: two steps, and
  its result is reused by run key.
- **Required**: the windowed result equals the whole-recording result (within float tolerance) for
  every step within its margin; nothing forces the whole into memory.

## B. lucem: tiles of a gigapixel slide

*A 100 000 × 80 000 px slide, three channels, a pyramid; cell detection with a 12 px blur.*

- **Walk**: tiles at a pyramid level with a margin of pixels, clipped at the slide's edges.
- **Labels across tiles**: cells cut by tile borders are joined after the walk. This is the domain's
  `finish` (stitching), not the contract's, but the contract must let a domain do work after the
  last piece.
- **Required**: tiled = whole for every step within its margin; labels stitched without seams.

## C. Previews while a setting moves

*Carla drags the threshold slider; the overlay updates on the part of the slide on screen.*

- **Preview**: one piece (the visible tile set), outputs returned, nothing stored, no run record
  ([spec §4.4](../spec/04-pipelines.md)).
- **Required**: a preview of a checked-but-unconfirmed script works; a run of it does not.
- **Open**: streamed previews (cancelling the previous one as the slider moves) in the runtime API.

## D. Curation: results made by people

*Ana accepts unit 17, rejects unit 18 as noise, merges 21 and 22.*

- **Manual steps**: each decision is a run of a manual step (`source`: the curation screen; `by`:
  Ana), whose output is a `dspk:curation` result linked to the sorting it judged
  ([spec §2.4](../spec/02-steps.md)).
- **History**: changing her mind is a new decision; the old one stays visible.
- **Two curators**: Ben's decisions on the same sorting are separate results; a comparison is a step
  over both.
- **Required**: "which units went into figure 3, and who accepted them" is answered from lineage.

## E. Across apps

*neuro-convert turns a vendor recording into NWB; dsp-kitchen sorts it; garnish runs statistics and
draws the figure.*

- Three apps, one lineage: each step's run record names its inputs by result ID, whichever app
  produced them.
- **Required**: the orchestrator runs the three as one pipeline, and `lineage` from the figure
  reaches the vendor file.
