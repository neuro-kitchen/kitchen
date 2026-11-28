# 0018. Compute below everything; libraries apart from apps

**Context.** Planning the next releases (neuro-convert with dspk's ops, dspk-live, device
repositories) showed two problems with the layout of decision 0014 and the architecture's four
layers. First, compute sat inside kitchen's runtime: a pure algorithm library (dspk's ops, lucem's
ops, a model) that only needs the GPU would have depended on all of kitchen (the shell, the server,
the web link), and released with it. Second, an app that is also a library invites cycles:
neuro-convert would use dspk's ops while dspk-synapse would use neuro-convert's readers. And the
word "runtime" named two things: CubeCL's backends (CUDA, wgpu, the CPU) and kitchen's back end.

**Decision.**

- **Compute is the bottom layer, its own repository after the split.** The CubeCL layer
  (backends, devices and their lifetime, launch geometry, autotune and pinning, timing; later the
  memory budgets and leases of dsp-kitchen and the canonical tiles and batches of lucem) depends on
  nothing of ours. It pins CubeCL and re-exports it: every crate reaches CubeCL through it, so one
  program never holds two CubeCL versions, and CubeCL is upgraded in one place. The GPU porting
  spec lives with it.
- **"Backend" in compute, "runtime" in kitchen.** CUDA, wgpu and the CPU are *backends*
  (`<PREFIX>_BACKEND`; the older `<PREFIX>_RUNTIME` is still read). Kitchen's *runtime* is the back
  end of an app: it takes a request, checks it against the contract, walks the data in pieces
  calling the libraries' ops, keeps the results with run records, and answers the runtime API.
- **Five layers, across repositories**, dependencies pointing down only:

  | Layer | Repositories | Depends on |
  |---|---|---|
  | compute | compute | CubeCL |
  | libraries | dspk (ops), lucem's ops, neuro-convert's readers and NWB writer, garnish's statistics, models | compute |
  | families | dspk-synapse; later EEG, EMG | libraries |
  | kitchen | contract, runtime, interface, deployment | compute |
  | apps | dsp-kitchen, lucem, neuro-convert, dspk-live, garnish | anything below |

- **Libraries never depend on apps; apps never depend on apps.** What two apps need moves down a
  layer. An app that holds a library splits: neuro-convert becomes its readers and NWB writer (a
  library) and its app.
- **Libraries stay pure.** They work on device buffers and do not depend on kitchen's contract;
  each app describes its ops to kitchen in its own domain layer. A library is usable from a
  notebook with no kitchen at all.

**Consequences.** Kitchen holds the contract, the runtime and the interface: what makes an app an
app. Compute changes rarely and is small, so the extra repository costs little; drift between
repositories is caught by each repository's CI listing the repositories it may depend on. The
first step is inside kitchen: `kitchen-compute` moves to `compute/`, a layer of its own below the
contract, then to its own repository.

**Alternatives.** Compute inside kitchen's runtime (one repository fewer; every library tied to
kitchen's releases). Compute inside each app (what dsp-kitchen and lucem had: two copies, two
upgrades). Libraries on kitchen's contract (ops described where they are written; every library
depends on kitchen).
