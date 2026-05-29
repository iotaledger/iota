# CI workflows: how to edit safely

**Read this before editing any workflow file here.** Some of these
workflows run PR-author-controlled code with elevated privileges. A small
change — a different trigger, a missing `persist-credentials: false`, an
unpinned action — can turn a safe workflow into a remote-code-execution or
secret-exfiltration hole, or silently make tests run against the wrong code.

Written for both humans and LLMs assisting with edits.

## The rules (apply all of them)

1. **Pin every third-party action to a full-length commit SHA**, never a
   tag. Tags are mutable; a compromised action repo can force-push a tag to
   malicious code and your workflow pulls it on the next run.
   ```yaml
   - uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd # v6.0.2  # good
   - uses: actions/checkout@v6                                                  # bad
   ```
   The SHA is the security boundary; the `# v6.0.2` comment is informational.
   When bumping, look up the new release's SHA in the action's repo and
   update both.

2. **Set `persist-credentials: false` on every checkout** that is followed
   by execution of repo code (`cargo test`, scripts, npm, ...). Otherwise
   `actions/checkout` leaves the auth token in `.git/config`, where that
   code can read it.

3. **Declare explicit minimal `permissions:`** on any workflow that runs
   untrusted code or holds write scopes. Listing `permissions:` overrides
   the (often too broad) default; unlisted scopes become `none`.

4. **Never interpolate PR-controlled values into a `run:` script.** PR
   title, body, `github.head_ref`, commit messages, labels are
   attacker-chosen. Pass them via `env:` and reference the shell variable:
   ```yaml
   - run: echo "Building $HEAD_REF"   # safe
     env:
       HEAD_REF: ${{ github.head_ref }}
   ```
   Direct `${{ github.head_ref }}` inside `run:` is shell injection. The
   same goes for parsing PR body in scripts: treat it as data, never `eval`
   it back into a shell.

5. **Don't combine `pull_request_target` with checking out / running PR
   code** (the "pwn request" pattern). If a flow genuinely must do both,
   it needs all of: minimal `permissions:`, `persist-credentials: false`,
   and human approval before execution.

## Triggers — pick the right one

| Event | `github.ref` points to | Workflow/script loaded from | Fork PR token/secrets |
| --- | --- | --- | --- |
| `pull_request` | merge-ref (PR HEAD) | PR HEAD | **read-only, no secrets** |
| `pull_request_target` | BASE branch | BASE | full default, secrets |
| `workflow_dispatch` | the dispatched ref | that ref | full default, secrets |

- **`pull_request`** — the default for "test/lint this PR". GitHub
  auto-restricts the token and hides secrets for fork PRs, which is what
  makes running PR code safe.
- **`pull_request_target`** — only for bot tasks that need write access and
  do **not** run PR code (labeling, commenting, dispatching). It loads the
  workflow/script from BASE, so a PR author can't alter the logic via their
  branch.
- **`workflow_dispatch`** — programmatic/manual. NOT fork-restricted, so it
  runs with full default token unless you narrow `permissions:`.

Key consequence: under `pull_request_target` and `workflow_dispatch`,
fork-PR protections are **off** — narrow `permissions:` yourself.

## The `head_sha` contract

`actions/checkout` defaults to `github.ref`. Under `pull_request` that's
the PR code, but under `workflow_dispatch` it's the dispatched ref
(`develop` for our flow) — **not the PR**.

So the heavy reusables (`_rust_tests.yml`, `_split_cluster*.yml`) take an
optional `head_sha` input, and callers **must** pass
`head_sha: ${{ inputs.head_sha || github.sha }}`. This applies both to
`actions/checkout` (`ref:`) and to any script arg that names the
commit-to-test. Forget it and the reusable silently tests the wrong code.
`nightly.yml` passes nothing — the `|| github.sha` fallback is correct
there since nightly runs on the ref it wants to test.

## Architecture

**Entry workflows** (`hierarchy.yml`, `heavy_tests.yml`, `nightly.yml`,
`ci_trigger.yml`, ...) have `on:` triggers and own their `concurrency:`.
**Reusables** (`_rust_tests.yml`, `_split_cluster.yml`, ...) are
`workflow_call`-only. A reusable's jobs run inside the caller's run and
share its `github.run_id`/context, so cancelling the caller cancels them —
that's why reusables don't define their own `concurrency:` (they can't
compute a correct key; it depends on the caller).

### On-demand heavy tests: `ci_trigger.yml` + `ci-trigger-dispatch.js`

`ci_trigger.yml` runs on `pull_request_target: [opened, edited]`. When a
maintainer ticks a checkbox in the PR body, `ci-trigger-dispatch.js`
unchecks it and dispatches the mapped workflow via `createWorkflowDispatch`
(`ref: 'develop'`). `heavy_tests.yml` then runs the heavy suite against the
PR's `head_sha`.

**Invariants — don't break these:**

- Keep `pull_request_target` (needs write for `pulls.update` +
  `createWorkflowDispatch`); the checkout has no `ref:` so the script
  always loads from BASE — a PR author can't swap the dispatcher logic.
- `CHECKBOX_DISPATCHES` is a closed allowlist (label → workflow file).
  Don't broaden it to substring/dynamic matching.
- Uncheck happens **before** dispatch — the load-bearing infinite-loop
  guard. Don't reorder.
- Permission gate: only `write`/`maintain`/`admin` senders dispatch. The
  maintainer's tick *is* the approval; a fork author ticking their own box
  unchecks but does not dispatch. (We use this instead of GitHub
  Environment approvals — code-only, no repo-settings change.)
- `createWorkflowDispatch` uses `ref: 'develop'` so the dispatched workflow
  is loaded trusted from BASE; the PR code is reached only via `head_sha`.
- `heavy_tests.yml` holds narrow `permissions: { contents: read, actions:
  write }` because it runs PR code under `workflow_dispatch` (not
  fork-restricted).

## Adding a new on-demand workflow

Four edits, all required:

1. **New workflow** — `on: workflow_dispatch` with `pr_number` (string,
   optional) and `head_sha` (string, required) inputs; minimal
   `permissions:`; every checkout uses
   `ref: ${{ inputs.head_sha || github.sha }}` + `persist-credentials: false`.
2. **`ci-trigger-dispatch.js`** — add `'Run X': 'x.yml'` to
   `CHECKBOX_DISPATCHES`.
3. **`ci_trigger.yml`** — extend the pre-filter `if:` with the new
   `contains(body, '- [x] Run X')` so the runner only spins up when needed.
4. **PR templates** in `.github/PULL_REQUEST_TEMPLATE/` — add the `- [ ] Run X`
   checkbox line.

Skipping step 2 fails safe (closed allowlist rejects the marker); skipping
3 just wastes runner time; skipping 4 means the box never appears.

## Changes that need extra review

`on:` trigger changes (esp. `pull_request` ↔ `pull_request_target`) ·
weakening `permissions:` · removing `persist-credentials: false` · adding
`ref: ${{ github.event.pull_request.head.* }}` to a `pull_request_target`
checkout · unpinning an action · adding `env:`/CLI secrets in the
heavy-tests path · editing the dispatcher's regex, allowlist, permission
gate, or uncheck ordering · dropping `head_sha` from a reusable's checkout
or script args.

## References

- [Secure use reference](https://docs.github.com/en/actions/reference/security/secure-use)
- [Reusing workflows](https://docs.github.com/en/actions/how-tos/reuse-automations/reuse-workflows)
