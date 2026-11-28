# Porting algorithms and models to CubeCL and Burn

*A spec sheet for translating an algorithm (a sorter, a filter, a segmentation's post-processing)
to CubeCL, and a trained model (PyTorch, Keras, TensorFlow, ONNX) to Burn on CubeCL, in any
neuro-kitchen app. Written 2026-11-28 from the ports done in lucem (StarDist, InstanSeg, Segment
Anything) and dsp-kitchen (Kilosort4, EMUsort), 2026-11-10 to 11-21, and from recent work on
interactive latency. Every principle names the commit or source it comes from.*

## What the ports achieved

The same RTX 2070 (8 GB, Turing), CUDA unless noted:

| Port | Before | After | What did it |
|---|---|---|---|
| InstanSeg, 512² image | 1179 ms | 111 ms | half precision; 3 × 3 convolutions as tensor-core matrix products; BatchNorm folded; post-processing in logits, a window row at a time, sparse overlaps (`7c53388`, `30972a2`) |
| InstanSeg, 2048² image | 21.7 s | 2.9 s | the same |
| InstanSeg network alone, 512² | 627 ms | 83 ms | convolutions as matrix products (`7c53388`) |
| StarDist, 512² / 4096² | 225 ms / 10.9 s | 39 ms / 3.3 s | half precision on the device and on the wire; 64+ channel layers as matrix products (`c8d07a5`) |
| StarDist/InstanSeg tile, canonical 512² | — | ~28–32 ms, ~250–350 MiB peak | one tile shape, batch 1 by default, idle model evicted (`31c097b`) |
| Kilosort4 (dspk) vs kilosort 4.1.3 (PyTorch) | 113 s solver, 5.4 GB GPU | 14.6 s solver, 3.8 GB GPU (wgpu: 49 s) | the whole sorter on the device; fused detection; precomputed kriging; no round trips (`86e5baa`, `0678191`, `908088e`, `90f9c8d`) |
| EMUsort (dspk) vs upstream (KS 4.0.18 + SpikeInterface) | 72 s solver | 9.1 s end to end (wgpu: 21.5 s) | the Kilosort4 stages, plus channel delays on the device |
| A processed view: adding an op to a 32-channel chain | 147–158 ms | 6–12 ms | chains share the uploaded window; nothing read or uploaded again (`1a8161e`) |

Parity was kept through every one of these (below, P10). The Kilosort4 port finds 128,325 spikes
and 283 units on the benchmark recording, against upstream's 137,374 and 263; that difference is
the open validation item, not noise to hide.

## The principles

### P1. Measure the whole path, phase by phase, cold and warm

Time from the request to the answer, split into phases: host preparation, upload, the network or
kernels, readback, post-processing, and what the caller does with the result. Report the first
run (compiling and autotuning) apart from the median of the next ones.

- *From:* InstanSeg's and StarDist's `bench` (per-phase `Timings`, cold and median; `b13b33d`,
  `c8d07a5`); Kilosort4's per-stage times against upstream's (`a86e578`); dsp-kitchen's
  `PreviewTimings` (`1a8161e`).
- *From FastFrames* (Apple and Cornell, 2026): the steps treated as fixed implementation details
  (the VAE decoder, KV-cache updates) took more than half of end-to-end latency (the decoder alone
  147 ms of 271.8 ms). The thing you came to port is often not the bottleneck; what surrounds it
  is. They measure response latency, from a new command to the first frame that reflects it.
- *Tools:* `kitchen_compute::bench::time_device` (device-synchronised), stage timers in the
  runner, a `bench` subcommand per model.

### P2. Replace the expensive surroundings, not only the core

When P1 shows the surroundings dominating, rethink them with the same care as the kernel: a
lighter equivalent with the same result.

- *From:* InstanSeg's post-processing rewritten (`30972a2`): the pixel classifier on logits, never
  the sigmoid (it is monotonic, so thresholds compare logits); a window row at a time so loops
  vectorise; overlaps only between windows that touch; hole filling on the mask's bounding box.
  1179 → 111 ms for the whole image, of which the network was 83.
- *From:* Kilosort4's drift pass (`0678191`): an amplitude-only detection that skips the PCA
  features and their download; the kriging matrices computed once per batch, not per pass
  (`90f9c8d`).
- *From FastFrames:* the VAE decoder replaced by a small learned head (147 ms → about 1 ms),
  the cache refresh by a light network (10× faster), quality unchanged.

### P3. Data stays on the device: upload once, read back once

Count every crossing between host and device. Each one should be the input arriving or the
answer leaving; nothing in between.

- *From:* zero-bounce pipeline ops (`d992add`): elementwise and mask ops on the device, where they
  had come back to the host between steps. Windows cropped on the device instead of downloaded
  and re-uploaded (`908088e`). Model weights uploaded once and activations kept in VRAM across
  conv → ReLU → linear (`6088035`). Chains of one recording share the uploaded window and read
  each other's rows on the device (`1a8161e`).
- *From:* lucem's served cells (`982aa61`): a page gets a 15–30 KB raster of the viewport,
  not 100,000+ polygons; the object table stays where it was computed.
- *Literature:* "data movement is the bottleneck" is the common thread of recent latency work:
  Hazy Research report that vLLM and SGLang use at most about 50% of an H100's memory bandwidth
  on a small model's decoding, the rest lost between kernels.

### P4. When bytes must move, move fewer

- Half precision on the device *and on the wire*: the tile is converted to f16 on the host and
  uploaded as f16; outputs come back as f16 (`c8d07a5`, `7c53388`). Half the bytes both ways,
  and the tensor cores take f16.
- Results, not raw data: a region read of a Zarr result, a raster, a summary (kitchen 4e;
  dsp-kitchen's `view::Summary`).
- Compress what crosses a network (kitchen-wire's chunks: lz4 or zstd).
- Keep f32 where it is faster or needed: the CPU runtime (half precision is slower there),
  reductions that accumulate (`f64` leaf sums in clustering, `908088e`).

### P5. Fuse to remove intermediates

A buffer written by one kernel and read by the next is a round trip through device memory; fuse
the two when the intermediate is not needed elsewhere.

- *From:* Kilosort4's neighbour maximum and temporal peak pooling fused through shared memory,
  removing the global `a_max` buffer (`908088e`); runs of pointwise stages fused into one kernel
  and a chain's trailing pointwise ops folded into the zoomed-out reduction (`1a8161e`);
  BatchNorm folded into the convolution at load (`7c53388`).
- *From:* fused seed selection and distance update in k-means++ (`6088035`); Louvain's counts
  cleared in place by the kernel that used them, not by a separate clear (`6088035`).
- *Literature:* megakernels (Hazy Research's Llama-1B; Mirage Persistent Kernel, OSDI 2026:
  up to 1.7× lower latency) take fusion to its limit by removing kernel boundaries altogether.
  A later paper finds that a few specialised fused kernels can beat a single megakernel, so fuse
  where the profile says the boundary costs, not everywhere. CubeCL's graph capture (a pipeline
  recorded once and replayed, `PipelineWorkspace::capture`, `1a8161e`) removes launch overhead
  without fusing.

### P6. Take the hardware's fast path, and keep a fallback

- When the library's kernel falls off the fast path, reshape the problem onto one that stays on
  it: Burn's implicit-GEMM convolution asked more shared memory than Turing has for 64+ channel
  layers and fell back to a direct kernel 15× slower; the same convolution as columns × weights
  ran on the tensor-core matrix product (`7c53388`). Only where needed: the 32-channel full-
  resolution layers stay native, avoiding the nine-fold copy of the largest activations.
- Check the shared-memory budget before choosing a tile size, and keep occupancy: Kilosort4 picks
  the largest `tile_y` that fits the unfused kernel first, so enabling fusion never lowers it on
  wgpu's 32 KB (`0678191`, `908088e`).
- Every fast path has a fallback that is tested: the direct convolution when the matrix product
  cannot launch (`a10718d`); the unfused kernels when the fused one does not fit (`908088e`); the
  CPU when the GPU is lost (`3276d4d`).
- FIR by FFT from 64 taps (`1a8161e`): the algorithm with the better complexity once the size
  warrants it.

### P7. Stable shapes: tune once, reuse forever

CubeCL autotunes and compiles per problem shape. A new shape mid-session is a stall.

- One canonical tile (StarDist and InstanSeg: 384 core + 64 halo = 512²); batches of 1 or the
  maximum only, tail batches padded (`4165505`, `lucem_core::compute::batch`).
- Tune over every shape inference will meet before a real image does (`tune`, `e34de72`,
  `b13b33d`; `lucem autotune`), with compiled kernels and decisions persisted
  (`kitchen_compute::configure_runtime`).
- Choices that change results are pinned for reproducible runs (`kitchen_compute::pinning`);
  previews pin a fixed FIR rule instead of benchmarking mid-zoom (`1a8161e`).

### P8. Memory is a budget, planned before allocating

- One device budget per process (half of what the device reports, overridable); room made before
  allocating; off-screen work released first (`1a8161e`, `3276d4d`).
- Buffers in size classes and reused at a high-water capacity, not allocated per call
  (`0678191`, `1a8161e`, lucem's buffer reuse `92e8eb4`); large tensors split (64 MiB blocks: 64
  Mi-value tiles exhausted wgpu on an 8 GB card at ~4 GB, `3276d4d`).
- Precomputation has a ceiling (`MAX_PRECOMPUTED_KRIGING_BYTES`, computed on demand beyond it,
  `90f9c8d`).
- One GPU, several uses: leases (Live, Interactive, Batch: `dspk_core::lease`, `17cdbb1`); an
  idle model evicted when another is chosen (`31c097b`); batch size by available memory
  (`LUCEM_ML_BATCH`, default 1 on shared 6–8 GB GPUs).

### P9. Keep the device busy: overlap and sync rarely

- Launch the next tile before reading the last one back (`1a8161e`); the reading thread allocates
  and fills buffers that move into the upload without a copy (`6088035`).
- Synchronise in batches, not per iteration: Louvain checks convergence every 4 rounds
  (`6088035`).
- *Literature:* the megakernel work names the gaps between dependent kernels ("bubbles") as the
  main loss at small batch; persistent work and fine-grained dependencies close them.

### P10. Exactness is part of the port

A port is a claim that it computes the same thing. State the tolerance, test it, keep the test.

- Parity fixtures from upstream, checked on every change: InstanSeg 336/336 and 25/25 through
  every optimisation; StarDist within 1.8e-3 (probability) and 0.1 px (distances), same nuclei
  (`c8d07a5`); Kilosort4's drift within ±0.5 µm of upstream, the same in 19 of 23 batches
  (`86e5baa`).
- Optimised and plain paths tested against each other bit for bit: fused against unfused
  pooling; precomputed against on-demand kriging; the drift-only detection against the full one
  (`908088e`, `90f9c8d`, `0678191`).
- Determinism designed in: tie-breaking order preserved when parallel order differs (`(s, k)`
  ties, `0678191`); summation order kept.
- Differences reported, not hidden (the spike counts above).

### P11. One implementation for every runtime

Kernels take a `Client` and never special-case the device (`kitchen-compute`'s rule): the same
code on CUDA, wgpu, the CPU and a browser's WebGPU (`aa83846`). Device-specific choices come from
measured properties (shared memory, plane width), not from the device's name.

### P12. Licences and provenance

Written from the paper and the published defaults; upstream's code read to match behaviour, never
copied when its licence differs (Kilosort4 and EMUsort: GPL-3.0, kept as the crate's licence;
`synapse/sorters/kilosort`). Weights fetched with pinned SHA-256 and their licence recorded
(StarDist BSD-3, InstanSeg Apache-2.0; `9215afe`).

## Spec: translating an algorithm to CubeCL

Each stage ends with its deliverable; a pull request per stage or two.

| Stage | Do | Deliverable |
|---|---|---|
| 0. Source | The paper, the published defaults, upstream's version and licence (P12) | A `NOTICE.md` and the module docs naming them |
| 1. Reference | Run upstream on a small fixture and a benchmark recording or image; keep its outputs | Fixtures and a parity test that fails today |
| 2. Baseline profile | Upstream's per-stage times, peak RAM and GPU memory on the benchmark (P1) | `benchmarks/<name>/results/upstream_*.json` |
| 3. Data-flow map | Every host ↔ device crossing in upstream's flow, with sizes; where intermediates live | A table in the module docs (P3, P4) |
| 4. Straight port | Stage by stage, on the device, parity passing per stage (P10, P11) | Parity green on CUDA, wgpu and the CPU |
| 5. Optimise, in order | Remove crossings (P3), then bytes (P4), then intermediates (P5), then fast paths (P6), then overlap (P9); re-measure after each (P1) | One commit per change, its numbers in the message |
| 6. Shapes | Canonical shapes and a `tune` over them (P7) | Cold and warm times; nothing tunes mid-run |
| 7. Memory | A budget, reuse, ceilings, the lease it takes (P8) | Peak GPU memory on an 8 GB card stated |
| 8. Benchmarks | Against upstream: per stage, end to end, peak memory, results compared (spikes, units, objects) | Results committed, docs updated |

Done when: parity holds within the stated tolerance on every runtime; the benchmark is committed
with upstream's beside it; it runs within an 8 GB GPU; nothing tunes or allocates in its steady
state.

## Spec: translating a model to Burn

| Step | Rule | From |
|---|---|---|
| Weights in | Read the published format in Rust (HDF5 for Keras, the TorchScript zip and pickle, safetensors): no Python or C library at run time. Fetch through the hub with a pinned SHA-256 | StarDist `convert`, InstanSeg `torchscript` |
| Converted once | Written as safetensors in the cache, tensors named as the Burn module's fields, the source's hash and the model's thresholds in the metadata; loaded with `SafetensorsStore` (and the PyTorch adapter) | `5fb5c76`, `162ac80` |
| The module | A `#[derive(Module)]` mirroring upstream's classes and layer names; shapes read from the weights (`UNetConfig::from_shapes`) | `instanseg::net` |
| Load-time transforms | Fold BatchNorm into convolutions in f32, then cast the network to f16 on a GPU; transpose weights once at upload | `7c53388`, `6088035` |
| Pre-processing | Exactly upstream's (percentiles, scaling, clamping, pixel size), in Rust, tested against upstream's arrays | StarDist, InstanSeg `predict` |
| Tiling | Canonical tile with halo, padded to the network's multiple; batch 1 or the maximum | `4165505`, P7 |
| Precision | f16 on the GPU and on the wire, f32 on the CPU; an environment variable to force either | `LUCEM_INSTANSEG_PRECISION` |
| Convolutions | Native where the library's fast path runs; as matrix products where it would fall back (measured per device), with the direct kernel as the tested fallback | `7c53388`, `a10718d` |
| Post-processing | In Rust, profiled like the network (P1, P2): monotonic transforms skipped, work bounded to what can change | `30972a2` |
| Interactive models | Run the heavy encoder once per image and keep its output; each interaction runs only the light decoder (Segment Anything 2: the embedding reused by key) | `lucem-model-sam` |
| Memory | Evict an idle model; a batch size set by the device's memory | `31c097b` |
| Shipping | A plugin per model (its wheel and binary), `bench` and `tune` commands, its licence checked | `565f9af`, `37614b2` |
| Parity | Upstream's outputs on fixtures, the tolerance stated per output, checked in CI on the CPU runtime | P10 |

ONNX Runtime is an interim, not a target: Segment Anything 2 runs on it today (`lucem-model-sam`) and is
the next model to bring to Burn by this table, its encoder first.

## Anti-patterns seen and fixed

- A readback to check a value or crop an array between two device steps (`908088e`, `d992add`).
- A shape per region of interest, re-tuned on every new crop (`4165505`).
- One fused kernel that lowers occupancy where it no longer fits (`0678191`).
- A tile so large it exhausts the device on wgpu (`3276d4d`).
- Two models resident when one is in use (`31c097b`).
- A sigmoid computed only to compare with a threshold (`30972a2`).
- A benchmark of the network alone when the image takes ten times longer (P1).

## Sources

- lucem commits `565f9af`, `7c53388`, `30972a2`, `c8d07a5`, `a10718d`, `5fb5c76`, `162ac80`,
  `e34de72`, `b13b33d`, `92e8eb4`, `4165505`, `31c097b`, `d992add`, `aa83846`, `982aa61`;
  `docs/.../learn/concepts/performance.md`.
- dsp-kitchen commits `86e5baa`, `0678191`, `908088e`, `90f9c8d`, `6088035`, `a86e578`,
  `1a8161e`, `3276d4d`, `17cdbb1`; `benchmarks/sorters/results/`.
- Chou et al., *FastFrames: Video Generation at Interactive Latency*, arXiv:2611.23226 (2026).
  Not read in full here: its figures come from the abstract and reports of it, and should be
  checked against the paper.
- Hazy Research, *Look Ma, No Bubbles! Designing a Low-Latency Megakernel for Llama-1B* (2025).
- Cheng et al., *MPK: A Compiler and Runtime for Mega-Kernelizing Tensor Programs*, OSDI 2026
  (arXiv:2512.22219).
- *One Megakernel is Not Enough: Predicting Where to Cut for Streaming Inference*
  (arXiv:2610.35336).
