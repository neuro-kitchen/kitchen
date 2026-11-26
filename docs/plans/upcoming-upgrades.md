# Upcoming upgrades (checked 2026-11-26, again that evening)

Three upgrades are due soon. gpui-kit 0.11 is done (0.0.9). The other two wait for stable
releases and are not worth holding phase 2 for: each lands when it is out, between phase 2's steps
and never inside the move-only one. One branch each, `chore/upgrade-<name>`, built with
`scripts/check-all.sh`.

| | We use | Latest stable | Coming |
|---|---|---|---|
| **gpui-kit** (our GPUI) | **0.11.0** in kitchen since 0.0.9 (lucem 0.10.0; dsp-kitchen pins `=0.10.0`) | 0.11.0 (2026-11-23); a release most Mondays | Follows GPUI's move to its own crate |
| **GPUI** (Zed) | through gpui-kit (its `gpui-pre-*` crates) | `gpui` 0.2.2 on crates.io, a year old | Zed is moving GPUI out of its monorepo into its own repository, with a fresh crates.io release announced for the coming weeks |
| **CubeCL** | 0.11.4 (kitchen-compute, dsp-kitchen, lucem) | 0.11.4 | 0.12.0-pre.2 (2026-11-09) |
| **Burn** | 0.22.1 (lucem `burn`, dsp-kitchen `burn-tensor`) | 0.22.1 | 0.23.0-pre.2 (2026-11-09) |

## Status, evening of 2026-11-26

- **gpui-kit 0.11: done** in kitchen 0.0.9 (no code changes needed; NCSA allowed for libfuzzer-sys).
- **Standalone GPUI: not out yet.** crates.io still has `gpui` 0.2.2 (2025-10-22); no new release
  from Zed. Check again on the first day of phase 2.
- **CubeCL 0.12 / Burn 0.23: still pre-releases** (pre.2, 2026-11-09). Stable 0.11.4 / 0.22.1.
- **Order with phase 2:** don't wait for either. Phase 2 starts on 0.11.4 / gpui-kit 0.11; each
  upgrade lands as its own pull request when its stable release is out, between phase 2's steps
  (never inside the move-only step, so a moved crate and a changed API are never one diff).

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

- crates.io API, 2026-11-26 (morning and evening, unchanged): `gpui` 0.2.2, `gpui-kit` 0.11.0, `cubecl` 0.11.4 / 0.12.0-pre.2,
  `burn` 0.22.1 / 0.23.0-pre.2.
- Zed's GPUI extraction: https://alphasignal.ai/news/zed-breaks-gpui-out-of-its-editor-monorepo-into-a-standalone-rust-package
  (not yet confirmed on Zed's own blog); https://github.com/zed-industries/zed/discussions/55271
- CubeCL releases: https://github.com/tracel-ai/cubecl/releases
- Burn releases: https://github.com/tracel-ai/burn/releases
