# Upcoming upgrades (checked 2026-11-26)

Three upgrades are due soon. Do them **after Phase 0** (the 0.0.9 branches built, tested and
merged on the current versions) and **before Phase 2** (the restructure into `contract/`,
`runtime/`, `interface/`), so that moved code is already on the new versions. One branch each,
`chore/upgrade-<name>`, built with `scripts/check-all.sh`.

| | We use | Latest stable | Coming |
|---|---|---|---|
| **gpui-kit** (our GPUI) | 0.10.0 (kitchen, lucem; dsp-kitchen pins `=0.10.0`) | 0.11.0 (2026-11-23) | Follows GPUI's move to its own crate |
| **GPUI** (Zed) | through gpui-kit (its `gpui-pre-*` crates) | `gpui` 0.2.2 on crates.io, a year old | Zed is moving GPUI out of its monorepo into its own repository, with a fresh crates.io release announced for the coming weeks |
| **CubeCL** | 0.11.4 (kitchen-compute, dsp-kitchen, lucem) | 0.11.4 | 0.12.0-pre.2 (2026-11-09) |
| **Burn** | 0.22.1 (lucem `burn`, dsp-kitchen `burn-tensor`) | 0.22.1 | 0.23.0-pre.2 (2026-11-09) |

## GPUI and gpui-kit

- When Zed publishes the standalone `gpui`, gpui-kit is expected to move onto it; we follow
  gpui-kit, not GPUI directly. Watch gpui-kit's releases.
- gpui-kit 0.11.0 is already out: upgrade to it first (a smaller step), then to the release on the
  standalone GPUI when it comes.
- What to check: kitchen-ui's theme mapping (`Theme` fields), the frame (`TitleBar`, buttons,
  menus, `dropdown_menu`), the test helpers (`TestWindowExt`, `try_find`, `press`, `click`), icon
  names (`IconName`, generated from the Lucide files), `canvas`, `RenderImage`.
- All three apps move together: they share kitchen-ui and kitchen-shell, so one GPUI per release.
- dsp-kitchen pins `=0.10.0`: unpin when it moves.

## CubeCL 0.12 and Burn 0.23

- They are released together and must move together: Burn 0.23 builds on CubeCL 0.12. lucem uses
  both, dsp-kitchen both (`burn-tensor`), kitchen-compute CubeCL.
- Wait for the stable releases (the 0.12 / 0.23 pre-releases are out as of 2026-11-09).
- What to check: kitchen-compute's runtime choice and device lifetime (`ComputeTarget`,
  `configure_runtime`, the `persistence` feature), launch geometry, autotune; the CPU runtime's
  LLVM; then dsp-kitchen's and lucem's kernels and their GPU tests (stage D, on a machine with a
  GPU or a Colab notebook).
- Burn 0.22 already removed the backend generic from models and tensors and replaced `ndarray`
  with `burn-flex`; read 0.23's notes for what follows from those.

## Sources

- crates.io API, 2026-11-26: `gpui` 0.2.2, `gpui-kit` 0.11.0, `cubecl` 0.11.4 / 0.12.0-pre.2,
  `burn` 0.22.1 / 0.23.0-pre.2.
- Zed's GPUI extraction: https://alphasignal.ai/news/zed-breaks-gpui-out-of-its-editor-monorepo-into-a-standalone-rust-package
  (not yet confirmed on Zed's own blog); https://github.com/zed-industries/zed/discussions/55271
- CubeCL releases: https://github.com/tracel-ai/cubecl/releases
- Burn releases: https://github.com/tracel-ai/burn/releases
