#!/usr/bin/env bash
# Builds and tests kitchen in stages, lightest first (docs/plans/0.0.9-frame.md, stage 2), and
# prints pass / skipped / failed per stage. Memory stays in check: one stage at a time, with
# CARGO_BUILD_JOBS (default 2) parallel jobs.
#
#   scripts/check-all.sh            stages A-C, and D when a GPU runtime is found
#   scripts/check-all.sh A B        only these stages
#   CHECK_ONLY=1 scripts/check-all.sh   `cargo check` instead of tests (no code generation)
#   CLEAN=1 scripts/check-all.sh    remove target/ between stages (when disk is short)
set -uo pipefail
cd "$(dirname "$0")/.."
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"

declare -A CRATES=(
  [A]="kitchen-contract kitchen-engine kitchen-scripts kitchen-store kitchen-services kitchen-plugin kitchen-tool kitchen-network kitchen-downloads"
  [B]="kitchen-ui kitchen-views kitchen-shell kitchen-editor kitchen-chat"
  [C]="kitchen-compute"
  [D]="kitchen-compute"
)
declare -A FEATURES=([A]="" [B]="" [C]="cpu" [D]="wgpu")
declare -A WHAT=(
  [A]="no GUI: contract, engine, scripts, store, services, plugins, tools, network, downloads"
  [B]="GUI: theme, views, frame, code, agent (needs the GPUI system libraries on Linux)"
  [C]="compute on the CPU runtime (downloads LLVM)"
  [D]="compute on the GPU (wgpu; add cuda by hand where there is CUDA)"
)

stages=("$@")
[ ${#stages[@]} -eq 0 ] && stages=(A B C D)

has_gpu() {
  command -v nvidia-smi >/dev/null 2>&1 && return 0
  command -v vulkaninfo >/dev/null 2>&1 && vulkaninfo --summary >/dev/null 2>&1 && return 0
  [ "$(uname)" = "Darwin" ] && return 0
  return 1
}

declare -A RESULT
for s in "${stages[@]}"; do
  [ -n "${CRATES[$s]:-}" ] || { echo "no stage $s (A B C D)"; exit 2; }
  if [ "$s" = D ] && ! has_gpu; then
    RESULT[$s]="skipped (no GPU found)"
    continue
  fi
  args=()
  for c in ${CRATES[$s]}; do args+=(-p "$c"); done
  [ -n "${FEATURES[$s]}" ] && args+=(--features "${FEATURES[$s]}")
  echo "== stage $s: ${WHAT[$s]}"
  if [ "${CHECK_ONLY:-0}" = 1 ]; then
    cmd=(cargo check --all-targets "${args[@]}")
  else
    cmd=(cargo test "${args[@]}")
  fi
  echo "   ${cmd[*]}"
  if "${cmd[@]}"; then RESULT[$s]="pass"; else RESULT[$s]="FAILED"; fi
  # The video crate's optional MP4 decoder rides with stage B
  if [ "$s" = B ] && [ "${RESULT[$s]}" = pass ] && [ "${CHECK_ONLY:-0}" != 1 ]; then
    cargo test -p kitchen-views --features h264 || RESULT[$s]="FAILED (kitchen-views h264)"
  fi
  df -h . | tail -1 | awk '{print "   disk: " $3 " used, " $4 " free"}'
  command -v free >/dev/null && free -h | awk '/Mem:/ {print "   memory: " $3 " used of " $2}'
  [ "${CLEAN:-0}" = 1 ] && cargo clean
done

echo
echo "== summary"
failed=0
for s in "${stages[@]}"; do
  printf "  %s  %-60s %s\n" "$s" "${WHAT[$s]}" "${RESULT[$s]}"
  [[ "${RESULT[$s]}" == FAILED* ]] && failed=1
done
exit $failed
