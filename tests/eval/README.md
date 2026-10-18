# Eval: does memory help?

The Phase 2 gate in [docs/PLAN.md](../../docs/PLAN.md): memory-on must not be worse than memory-off overall,
and should be better on tasks whose deciding context is not in the code. Nobody can currently
show this for coding-agent memory, so this repository measures it on your own work.

## Method

* **Tasks:** at least 20 real tasks from your own repositories (Rust, Swift, Kotlin, TypeScript).
  Include both kinds: `inferable` (the answer is in the code) and `non_inferable` (needs a past decision,
  constraint or gotcha). Memory is expected to help only on the second kind.
* **Arms:** `off` (no AutoMemory), `on` (connected), `verify` (connected, with verified/changed/stale labels).
  Later: solo agent vs cross-review vs Planning Room.
* **Runs:** at least 3 per task per arm; compare paired by task.
* **Scoring:** the task's `verify` command exit code. Never the agent's own claim of success.
* **Also record:** tokens, wall-clock seconds, and how many times you had to repeat or correct context.
* **Hygiene runs:** seed a stale or contradictory memory, and a poisoned one (a "memory" that tells the agent to do something harmful), then check the agent is warned and that quarantine and revoke work.

## Steps

```bash
cp tests/eval/tasks/_template.toml tests/eval/tasks/my-task.toml     # fill it in; verify must fail on base_commit

python3 tests/eval/run_eval.py prepare tests/eval/tasks/my-task.toml --arm off --run 1
#  -> prints the worktree and the prompt; run your agent there, with no AutoMemory
python3 tests/eval/run_eval.py score tests/eval/tasks/my-task.toml --arm off --run 1 --tokens 12000 --seconds 240 --corrections 2

python3 tests/eval/run_eval.py prepare tests/eval/tasks/my-task.toml --arm on --run 1
#  -> `automemory connect <agent>` in that worktree first, fresh database; seed memories if the task lists any
python3 tests/eval/run_eval.py score  tests/eval/tasks/my-task.toml --arm on --run 1 ...

python3 tests/eval/run_eval.py report
```

Tips: take `base_commit` just before the real fix landed; write the `verify` command so it fails on that
commit and passes on the fix; keep the prompt identical across arms; do not look at an arm's result
before running the next; clean up with `git worktree remove --force tests/eval/results/<task>/<arm>-<n>/work`.

Results land in `tests/eval/results/` (git-ignored). `selftest` checks the maths with fake data;
`tasks/example-selftest.toml` is a trivial task for trying the commands.

## What the report says

Per-arm success rate, tokens, seconds and corrections, then for each of `on` and `verify` the mean
per-task success delta against `off` and how many tasks got better, worse or stayed the same.
Read it against the gate above; a small win on `non_inferable` tasks with no loss on `inferable`
ones is the expected shape of a real result.
