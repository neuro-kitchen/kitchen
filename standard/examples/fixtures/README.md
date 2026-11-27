# Fixtures

The examples as data, read and checked by kitchen-contract's tests
(`contract/kitchen-contract/tests/examples.rs`): what each example's walkthrough says on paper,
checked on every pull request.

| File | Example |
|---|---|
| `dspk-ops.json` | dsp-kitchen's ops as the catalog describes them: built in, a tool (Kilosort, version 4), a script op in the workspace (01), an op with effects (DANDI upload) |
| `lucem-ops.json` | lucem's ops: a `finish` that stitches labels across tiles, a model that is not installed |
| `03-cell-detect.pipeline.json` | 03: denoise → threshold → watershed → measure, saved as a recipe |
| `04a-windows.pipeline.json` | 04 A: high-pass → reference → whitening (fitted) → detection, walked in windows |
| `04e-across-apps.pipeline.json` | 04 E: another app's op in dsp-kitchen's pipeline, refused: it is a chain, run by the orchestrator |
