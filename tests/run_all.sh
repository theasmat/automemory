#!/usr/bin/env bash
# Runs every automated test. From the repo root:  tests/run_all.sh [--quick]
#   --quick   skip the release build, the latency run and the CodeIntel smoke test
# Environment: AUTOMEMORY_BIN to test a specific binary; AUTOMEMORY_CBM_BIN for the engine.
set -u
cd "$(dirname "$0")/.."
QUICK=0; [ "${1:-}" = "--quick" ] && QUICK=1
FAILED=()

run() {  # run <label> <command...>
  local label="$1"; shift
  echo; echo "━━ $label"
  if "$@"; then echo "   ✓ $label"; else echo "   ✗ $label"; FAILED+=("$label"); fi
}

if [ "$QUICK" = 0 ] && [ -z "${AUTOMEMORY_BIN:-}" ]; then
  run "build release binary" cargo build --release -q
fi
[ -z "${AUTOMEMORY_BIN:-}" ] && { [ -x target/release/automemory ] && export AUTOMEMORY_BIN="$PWD/target/release/automemory" || export AUTOMEMORY_BIN="$PWD/target/debug/automemory"; }
echo "binary under test: $AUTOMEMORY_BIN"

run "rust unit + integration tests"   cargo test --workspace
run "agent hook contract"             python3 tests/contract/run.py
run "guardrail table"                 python3 tests/guardrails/run.py
run "two-agent collision"             python3 tests/scenarios/collision.py
run "automatic locking v2"          python3 tests/scenarios/locking_v2.py
run "session lifecycle"               python3 tests/scenarios/session_lifecycle.py
run "two machines (git vault)"        python3 tests/scenarios/two_machines.py
run "embeddings modes"                python3 tests/scenarios/embeddings_modes.py
run "shared instruction files"        python3 tests/scenarios/rules_export.py
run "registration + hook robustness"  python3 tests/scenarios/registration.py
run "human password gate"             python3 tests/scenarios/human_gate.py
run "dashboard API"                   python3 tests/scenarios/dashboard.py
run "onboarding checklist"            python3 tests/scenarios/onboarding.py
run "guided tour and demo"            python3 tests/scenarios/tour.py
run "dashboard UI contrast"           python3 tests/ui/contrast.py
run "eval harness maths"              python3 tests/eval/run_eval.py selftest
if [ "$QUICK" = 0 ]; then
  run "CodeIntel smoke (real engine)" python3 tests/codeintel/smoke.py
  run "hook latency"                  python3 tests/perf/hook_p95.py 200 50
fi
if [ -d workers/sync-gateway/node_modules ]; then
  run "sync-gateway webhook tests"    bash -c 'cd workers/sync-gateway && npx vitest run test/webhook.test.ts'
else
  echo; echo "━━ sync-gateway webhook tests: skipped (run: cd workers/sync-gateway && pnpm install)"
fi

echo
if [ ${#FAILED[@]} -eq 0 ]; then echo "ALL AUTOMATED TESTS PASSED"; echo "Still manual: tests/manual/LIVE_CHECKLIST.md, and the eval runs in tests/eval/"; exit 0; fi
echo "FAILED: ${FAILED[*]}"; exit 1
