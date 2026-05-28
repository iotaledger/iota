# CI workflows: design and security guide

This directory contains GitHub Actions workflows for `iotaledger/iota`.
**Read this guide before editing any workflow file.** Several of these
workflows interact with GitHub's security model in subtle ways — small
changes (a different trigger, a missing `persist-credentials: false`, an
unpinned action) can introduce critical vulnerabilities or quietly break
the intended invariants.

The guide is written for both humans and LLMs assisting with edits.

## Table of contents

- [Why this matters](#why-this-matters)
- [Trigger semantics](#trigger-semantics)
- [Permissions](#permissions)
- [Checkout settings](#checkout-settings)
- [Action pinning](#action-pinning)
- [Untrusted input handling](#untrusted-input-handling)
- [Our architecture](#our-architecture)
- [Dangerous changes — review carefully](#dangerous-changes--review-carefully)
- [How to add a new on-demand workflow](#how-to-add-a-new-on-demand-workflow)
- [References](#references)

## Why this matters

GitHub Actions has been the target of multiple high-profile attacks in
2024–2026:

- **tj-actions/changed-files** (Mar 2025, [CVE-2025-30066](https://github.com/advisories/ghsa-mrrh-fwg8-r2c3))
  — attacker rewrote version tags to point at malicious commit;
  ~23,000 repos exposed CI secrets in logs.
- **Trivy / trivy-action** (Feb–Mar 2026) — `pull_request_target` workflow
  exploited to exfiltrate PAT; 75 of 76 version tags force-pushed.
- **Ultralytics YOLO** (Dec 2024) — branch-name injection into `run:`
  under `pull_request_target` led to RCE with secrets access.
- **PostHog (Shai Hulud v2)** — `pull_request_target` workflow exploited
  to leak npm publishing token; compromised SDK packages published.
- **Spotbugs**, **Gluestack** ([CVE-2025-53104](https://www.sysdig.com/blog/cve-2025-53104-command-injection-via-github-actions-workflow-in-gluestack-ui)),
  **Langflow** ([GHSA-87cc-65ph-2j4w](https://github.com/langflow-ai/langflow/security/advisories/GHSA-87cc-65ph-2j4w))
  — various injection patterns through `pull_request_target`.

Most of these exploited patterns that look innocent in isolation but are
dangerous in combination — especially `pull_request_target` plus
checkout-of-PR-code, or unpinned actions. The sections below are the
playbook for avoiding each pattern.

## Trigger semantics

Three event types matter:

| Event                 | When it fires                     | `github.ref` resolves to             | Workflow file loaded from | Fork PR: token / secrets             |
| --------------------- | --------------------------------- | ------------------------------------ | ------------------------- | ------------------------------------ |
| `pull_request`        | PR opened / sync / etc.           | merge-ref (`refs/pull/<N>/merge`)    | PR HEAD                   | **Read-only**, no secrets            |
| `pull_request_target` | same PR events                    | BASE ref (e.g. `refs/heads/develop`) | BASE                      | **Full default**, secrets accessible |
| `workflow_dispatch`   | API call or "Run workflow" button | The `ref` passed at dispatch         | That ref                  | **Full default**, secrets accessible |

### When to use each

- **`pull_request`** — most workflows. Runs PR code in a safe sandbox.
  Suitable for "test this PR's code", linting, etc. GitHub auto-restricts
  permissions/secrets for fork PRs, which is what makes this safe.
- **`pull_request_target`** — ONLY for bot-like tasks that need write
  permissions and DO NOT execute PR-author-controlled code. Examples:
  labeling, commenting, dispatching other workflows. Never `actions/checkout`
  PR HEAD here without explicit safeguards — that defeats the purpose.
- **`workflow_dispatch`** — programmatic or manual triggers. Inherits the
  repo's default GITHUB_TOKEN permissions; NOT fork-restricted. Use when
  one workflow dispatches another (e.g., `ci_trigger.yml` →
  `heavy_tests.yml`) or for manual maintainer-driven runs.

### The "pwn request" pattern

A workflow that uses `pull_request_target` AND checks out / executes
PR-author-controlled code lets an attacker run arbitrary code with full
repo permissions and secret access. This is the pattern behind most
recent breaches. To avoid:

- Don't combine `pull_request_target` with PR-code execution.
- If you must (we do, via `ci_trigger.yml` → `heavy_tests.yml`):
  - Narrow `permissions:` to the minimum.
  - Set `persist-credentials: false` on every checkout.
  - Require human approval before code execution (we use the
    maintainer-tick gate in `ci-trigger-dispatch.js`).

## Permissions

### Principle of least privilege

Set `permissions:` explicitly on every workflow that touches anything
sensitive. The repo's default GITHUB_TOKEN permissions are often broader
than necessary. Declaring `permissions: { ... }` overrides the default;
unlisted scopes become `none`.

For workflows that run PR-author-controlled code, narrow permissions are
critical — they limit the blast radius if the token is exfiltrated. Our
`heavy_tests.yml` is a good example:

```yaml
permissions:
  contents: read # for actions/checkout + dorny/paths-filter
  actions: write # for the rust-tests-cancel / rust-simtests-cancel jobs
```

Everything else (`pull-requests`, `issues`, `packages`, ...) is `none`.

### How GitHub restricts token / secrets for fork PRs

For `pull_request` events from forks, GitHub automatically:

- Forces GITHUB_TOKEN to **read-only**, regardless of repo settings or
  `permissions:` declarations.
- Makes `${{ secrets.X }}` evaluate to empty.

This protection is **bypassed** by `pull_request_target` and
`workflow_dispatch`. If you use those for fork PRs, the workflow runs with
full default privileges — narrow `permissions:` explicitly to compensate.

### Token leakage paths

A token can be exposed to running code via:

- **`actions/checkout`** stores the token in `.git/config` by default.
  Any code that runs after checkout can read it. Mitigation:
  `persist-credentials: false`.
- **Environment variables**: any step with `env: { TOKEN: ${{ github.token }} }`
  exposes the token in `process.env` of every subprocess.
- **Command-line arguments**: `gh ... --token=${{ github.token }}` puts
  the token in `ps` / `/proc/<pid>/cmdline`.

Only use these paths where strictly necessary, and only where the code
running afterwards is trusted.

## Checkout settings

### `persist-credentials: false`

Set this **whenever the checkout will be followed by execution of code
that may be untrusted** (`cargo test`, npm scripts, shell scripts from
the repo, etc.). It prevents the auth token from being written to
`.git/config`.

We use it everywhere in the heavy-tests path: `heavy_tests.yml.diff`,
`crates-changes`, `external-changes`, every checkout in `_rust_tests.yml`
and `_split_cluster*.yml`. The composite `.github/actions/diffs/action.yml`
also sets it.

### `ref:` and the `head_sha` contract

`actions/checkout` defaults to `github.ref`. This works correctly under
`pull_request` (ref = merge-ref includes PR code) but **NOT** under
`workflow_dispatch` (ref = the dispatched ref, which is `develop` for our
flow — not the PR).

The heavy reusables (`_rust_tests.yml`, `_split_cluster*.yml`) declare an
optional `head_sha` input. Callers **MUST** pass
`head_sha: ${{ inputs.head_sha || github.sha }}` when invoking them so
the inner checkout grabs the right code. `heavy_tests.yml` does this for
its workflow_dispatch path; `nightly.yml` passes nothing (the input
defaults are empty → fallback to `github.sha` = whatever ref nightly ran
on, which is correct for the schedule/manual flow).

If you add a new caller of one of these reusables and forget to pass
`head_sha`, the reusable will check out the wrong code (and your tests
will silently pass on the wrong revision).

## Action pinning

**SHA-pin every third-party action.** Version tags are mutable — an
attacker who compromises an action's repository can force-push the tag to
point at malicious code, and your workflow will silently pull it on the
next run. This is what happened in the tj-actions and trivy-action
incidents.

```yaml
# Good — full-length commit SHA + version comment for humans:
- uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd # v6.0.2

# Bad — mutable tag, can be force-pushed:
- uses: actions/checkout@v6
- uses: actions/checkout@v6.0.2
```

The SHA is the security boundary. The `# v6.0.2` comment is for humans /
LLMs and is informational only.

When bumping an action's version: look up the release tag's SHA in the
action's repo, verify it matches a published release, and update both the
SHA and the comment.

## Untrusted input handling

PR-author-controlled values include the PR title, body, branch name
(`github.head_ref`), commit messages, label values, etc. These can be any
string an attacker chooses.

**Never interpolate them into a `run:` shell step:**

```yaml
# DANGEROUS — command injection via crafted branch name:
- run: echo "Building ${{ github.head_ref }}"
```

**Always pass them through `env:`:**

```yaml
- run: echo "Building $HEAD_REF"
  env:
    HEAD_REF: ${{ github.head_ref }}
```

Real CVEs from this pattern: [CVE-2025-53104 (gluestack-ui)](https://www.sysdig.com/blog/cve-2025-53104-command-injection-via-github-actions-workflow-in-gluestack-ui),
[GHSA-87cc-65ph-2j4w (langflow)](https://github.com/langflow-ai/langflow/security/advisories/GHSA-87cc-65ph-2j4w),
[GHSA-xr6r-vj48-29f6 (Roo-Code)](https://github.com/RooCodeInc/Roo-Code/security/advisories/GHSA-xr6r-vj48-29f6).

The same applies to PR body parsing in scripts: treat it as data, never
as code. Our `ci-trigger-dispatch.js` only does regex matching and string
replacement on the body — never `eval`, `new Function`, or template
substitution back into shell.

## Our architecture

### Entry workflows vs reusables

| Type     | Examples                                                                           | Has trigger events                        | Owns concurrency     |
| -------- | ---------------------------------------------------------------------------------- | ----------------------------------------- | -------------------- |
| Entry    | `hierarchy.yml`, `heavy_tests.yml`, `nightly.yml`, `ci_trigger.yml`, `pr_lint.yml` | Yes (`on: pull_request`, `schedule`, ...) | Yes (workflow-level) |
| Reusable | `_rust_tests.yml`, `_split_cluster.yml`, `_rust_lints.yml`, `_typos.yml`, ...      | Only `workflow_call`                      | No (caller owns it)  |

A reusable workflow's jobs run inside the caller's `workflow_run`. They
share the same `github.run_id` and contexts. Cancelling the caller
cancels everything inside.

This is why reusables don't (and shouldn't) define their own
`concurrency:` — they can't compute a correct key (it depends on the
caller's trigger). Concurrency belongs at entry-workflow level.

### ci_trigger.yml + ci-trigger-dispatch.js (the dispatcher)

Triggered by `pull_request_target: [opened, edited]`. Logic in
`.github/scripts/ci-trigger-dispatch.js` (loaded via `actions/checkout` +
`require`). When a user ticks a checkbox in the PR description whose
label matches the `CHECKBOX_DISPATCHES` map, the dispatcher unchecks the
box, then dispatches the mapped workflow via `createWorkflowDispatch`.

**Security invariants (don't break these without thinking):**

1. **`pull_request_target`** — needs write permissions for `pulls.update`
   and `createWorkflowDispatch`. Don't switch to `pull_request`.
2. **`actions/checkout` without `ref:`** — pulls the dispatcher script
   from the BASE branch. A PR author cannot replace the dispatcher logic
   by editing this file in their PR branch.
3. **Pre-filter `if:`** — `contains(body, '- [x] Run heavy tests')`. Keep
   this in sync with `CHECKBOX_DISPATCHES` so the runner only spins up
   for relevant edits.
4. **Uncheck-before-dispatch** — guarantees no infinite loop even if a
   later step fails. GitHub also suppresses workflow runs from
   `GITHUB_TOKEN`-triggered events, but the uncheck-first ordering is the
   load-bearing defense; don't rearrange.
5. **`CHECKBOX_DISPATCHES` is a whitelist** — closed by default. A
   marker pointing at any other workflow file would be rejected. Don't
   replace this with substring matching or anything that broadens it.
6. **Permission gate on `sender.login`** — only collaborators with
   `write`/`maintain`/`admin` may dispatch. A fork-PR author ticking
   their own box does NOT trigger (their box is unchecked above, but no
   dispatch happens). A maintainer ticking the box is the approval action.

### heavy_tests.yml (the heavy-tests entry point)

Triggered by `workflow_dispatch` (from `ci_trigger.yml`) and `push` to
long-lived branches. Has narrow `permissions: { contents: read, actions:
write }`. Passes `head_sha` to its reusables so they check out the PR's
code, not develop.

Concurrency keyed per-PR for workflow_dispatch (newer dispatch supersedes
older) and per-branch for push (develop queues to test every commit,
testnet/mainnet/release cancel-in-progress).

### nightly.yml

A sibling entry point. Schedule-triggered (daily) plus manual
`workflow_dispatch`. Calls the same reusables as `heavy_tests.yml` plus
its own jobs (examples, cargo-deny, simtest). Does **not** go through
`heavy_tests.yml`; the two are siblings because they have different
parameter expectations, permissions, and triggers.

### Permission gate vs Environment approval

We use a permission-based gate in `ci-trigger-dispatch.js` (sender must
have write+) instead of GitHub Environments with required reviewers.
Both achieve "maintainer approves heavy runs"; the permission gate is
simpler (code-only, no repo settings change) at the cost of less native
approval UX. If you want explicit "Approve and run" buttons in the
Actions UI, switch to Environment approvals on heavy_tests.yml.

## Dangerous changes — review carefully

Any of these in a PR deserves extra scrutiny — they're the patterns that
turn safe workflows into vulnerabilities:

- **Changing `on:` triggers** — especially `pull_request` ↔
  `pull_request_target`. Tiny diff, huge security implications.
- **Removing or weakening `permissions:`** — defaults are often too broad.
- **Removing `persist-credentials: false`** — exposes the token to
  running code via `.git/config`.
- **Adding `ref: ${{ github.event.pull_request.head.X }}` to checkout in
  a `pull_request_target` workflow** — the classic pwn-request pattern.
- **Unpinning an action** (tag instead of full SHA) — opens the door to
  tag-rewrite supply chain attacks.
- **Adding `env: { X: ${{ secrets.Y }} }` anywhere in the heavy-tests
  path** — exposes the secret to PR-author code.
- **Editing `ci-trigger-dispatch.js`** — particularly the regex pattern,
  the permission check, `CHECKBOX_DISPATCHES`, or the
  uncheck-before-dispatch ordering. Whitelist + permission gate semantics
  must be preserved.
- **Adding a new dispatchable workflow** without updating
  `CHECKBOX_DISPATCHES`, the `if:` pre-filter, and the PR template
  (see next section).
- **Changing the `ref:` on a reusable's checkout** — must keep the
  `${{ inputs.head_sha || github.sha }}` pattern; otherwise heavy tests
  may quietly test the wrong code.

## How to add a new on-demand workflow

Four places to touch:

1. **The new workflow** (e.g., `e2e_tests.yml`):
   - `on: workflow_dispatch:` with `pr_number` (string, optional) and
     `head_sha` (string, required) inputs.
   - Narrow `permissions:` — start from nothing and add only what's needed.
   - Every `actions/checkout`: `ref: ${{ inputs.head_sha || github.sha }}`,
     `persist-credentials: false`.

2. **`.github/scripts/ci-trigger-dispatch.js`** — extend
   `CHECKBOX_DISPATCHES`:

   ```js
   const CHECKBOX_DISPATCHES = {
     'Run heavy tests': 'heavy_tests.yml',
     'Run e2e tests': 'e2e_tests.yml',   // new
   };
   ```

3. **`ci_trigger.yml`** — extend the pre-filter `if:` so the dispatcher
   spins up only for relevant edits:

   ```yaml
   if: >-
     github.event.sender.type != 'Bot'
     && (contains(github.event.pull_request.body, '- [x] Run heavy tests')
         || contains(github.event.pull_request.body, '- [x] Run e2e tests'))
   ```

4. **PR templates** in `.github/PULL_REQUEST_TEMPLATE/` — add the matching
   checkbox line, e.g. `- [ ] Run e2e tests`, to each sub-template that
   contributors use.

If you skip step 2, the dispatcher will reject the marker (allowlist is
closed by default — failure mode is safe). If you skip step 3, the
dispatcher will run for unrelated description edits (extra runner cost,
not a security issue). If you skip step 4, the checkbox won't appear in
new PRs.

## References

### GitHub Actions documentation

- [Secure use reference](https://docs.github.com/en/actions/reference/security/secure-use)
- [Secure use reference](https://docs.github.com/en/actions/reference/security/secure-use)
- [Reusing workflows](https://docs.github.com/en/actions/using-workflows/reusing-workflows)
- [Events that trigger workflows](https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows)

### Notable breaches & writeups

- [tj-actions/changed-files (CVE-2025-30066)](https://github.com/advisories/ghsa-mrrh-fwg8-r2c3)
- [Wiz — Hardening GitHub Actions: Lessons from Recent Attacks](https://www.wiz.io/blog/github-actions-security-guide)
- [Snyk — Trivy supply chain compromise](https://snyk.io/articles/trivy-github-actions-supply-chain-compromise/)
- [Orca — pull_request_nightmare Part 2](https://orca.security/resources/blog/pull-request-nightmare-part-2-exploits/)
- [Endor Labs — Lessons from Trivy](https://www.endorlabs.com/learn/github-actions-security-lessons-from-trivy)
- [Unit 42 — tj-actions Supply Chain Attack](https://unit42.paloaltonetworks.com/github-actions-supply-chain-attack/)

### Linting / static analysis tools (consider adding)

- [zizmor](https://github.com/zizmorcore/zizmor) — GitHub Actions linter
  catching common security issues (unpinned actions, dangerous
  triggers, script injection).
- [OpenSSF Scorecard](https://github.com/ossf/scorecard) — automated
  security review of repositories, including Actions checks.

### Other security-related cheat-sheets

- [GitGuardian — GitHub Actions Security Cheat Sheet](https://blog.gitguardian.com/github-actions-security-cheat-sheet/)
- [Corgea — GitHub Actions Security Checklist](https://corgea.com/learn/github-actions-security-checklist)
- [Aikido — Security Checklist for GitHub Actions](https://www.aikido.dev/blog/checklist-github-actions)
