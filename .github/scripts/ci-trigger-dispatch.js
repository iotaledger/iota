// Dispatcher logic for `.github/workflows/ci_trigger.yml`. Extracted to a
// separate file for proper JavaScript tooling (syntax highlighting, lint,
// format). Invoked from the workflow via `actions/github-script` + `require`.
//
// Security model: the workflow uses `pull_request_target` and the
// `actions/checkout` step there grabs the BASE ref by default, so this file
// is always loaded from `develop` — a PR author can't replace the dispatcher
// logic by editing this file in their PR branch. See the top-of-file comment
// in `ci_trigger.yml` for the full design / security notes.

// Closed-by-default map from a checkbox's visible label text to the workflow
// file dispatched when the box is ticked. Any other checkbox in the body
// (Basic tests, Release Notes, ...) is ignored. When adding an entry, also
// extend the top-level `if:` filter in `ci_trigger.yml` and add the matching
// `- [ ] <label>` line to the PR sub-template(s).
const CHECKBOX_DISPATCHES = {
  'Run heavy tests': 'heavy_tests.yml',
};

// Escape a literal string for use inside a regex.
const reEscape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

module.exports = async ({ github, context, core }) => {
  const pr = context.payload.pull_request;
  const body = pr.body || '';

  // For each known label: find its checked-line occurrence
  // (`^- [x] <label>[ \t]*$`), queue it, and uncheck it in the working
  // body. We collect everything first and PUT once below.
  const triggered = [];
  let newBody = body;
  for (const [label, workflow] of Object.entries(CHECKBOX_DISPATCHES)) {
    const re = new RegExp(
      `^- \\[x\\] ${reEscape(label)}[ \\t]*$`,
      'gm',
    );
    if (!re.test(newBody)) continue;
    triggered.push({ label, workflow });
    newBody = newBody.replace(re, `- [ ] ${label}`);
  }

  if (triggered.length === 0) {
    core.info('No known ci-trigger checkbox ticked; nothing to do.');
    return;
  }

  // Step 1: uncheck every triggered box in a single PUT, BEFORE dispatching
  // anything. This guarantees no infinite loop — after our edit no
  // `[x] <known-label>` line exists, so even if the resulting `edited`
  // event reached us we'd no-op. (GitHub already suppresses workflow runs
  // for events triggered by GITHUB_TOKEN, which is the primary loop guard;
  // this is belt-and-suspenders.)
  await github.rest.pulls.update({
    owner: context.repo.owner,
    repo: context.repo.repo,
    pull_number: pr.number,
    body: newBody,
  });
  core.info(`Unchecked ${triggered.length} ci-trigger checkbox(es).`);

  // Permission gate: only repository collaborators with write+ access may
  // actually dispatch. The tick of the checkbox itself is treated as the
  // approval action — a maintainer ticking the box (their own PR or someone
  // else's) means "I approve this run". A fork-PR author ticking their own
  // checkbox is unchecked above but does NOT dispatch — a maintainer has to
  // come in and tick it themselves to authorize. We err on the side of
  // skipping when the API call fails (no permission → no dispatch).
  const senderLogin = context.payload.sender.login;
  const WRITE_PERMISSIONS = new Set(['admin', 'maintain', 'write']);
  let senderPermission = null;
  try {
    const { data } = await github.rest.repos.getCollaboratorPermissionLevel({
      owner: context.repo.owner,
      repo: context.repo.repo,
      username: senderLogin,
    });
    senderPermission = data.permission;
  } catch (err) {
    core.warning(
      `Couldn't fetch collaborator permission for @${senderLogin}: ${err.message}. ` +
        `Treating as no write access.`,
    );
  }
  if (!WRITE_PERMISSIONS.has(senderPermission)) {
    core.notice(
      `Skipping dispatch: @${senderLogin} (permission: ${senderPermission ?? 'unknown'}) ` +
        `does not have write access. A repository maintainer needs to tick the ` +
        `checkbox to dispatch this workflow.`,
    );
    return;
  }

  // Step 2: dispatch each matched workflow. We continue past individual
  // failures so a transient error on one doesn't block others.
  //
  // Before dispatching we also check whether a run of the same workflow
  // already exists for the same SHA in a non-terminal state. If so we skip
  // — the user just re-ticked while a previous request is still being
  // handled, and we'd rather let in-flight progress finish than restart
  // from scratch via heavy_tests.yml's `cancel-in-progress`.
  const headSha = process.env.HEAD_SHA;
  const prNumber = process.env.PR_NUMBER;
  let failed = 0;
  for (const { label, workflow } of triggered) {
    // Best-effort active-run check. If the API call fails for any reason
    // (workflow not on default branch yet, transient 5xx, rate limit) we
    // fall through and dispatch anyway — better than silently dropping a
    // user-requested run.
    let activeRun = null;
    try {
      const { data } = await github.rest.actions.listWorkflowRuns({
        owner: context.repo.owner,
        repo: context.repo.repo,
        workflow_id: workflow,
        head_sha: headSha,
        per_page: 10,
      });
      activeRun = data.workflow_runs.find((r) => r.status !== 'completed');
    } catch (err) {
      core.warning(
        `Couldn't list existing runs of ${workflow} for SHA ${headSha}: ${err.message}. ` +
          `Proceeding with dispatch.`,
      );
    }
    if (activeRun) {
      core.notice(
        `${workflow} is already active for ${headSha} ` +
          `(run #${activeRun.id}, status: ${activeRun.status}) — skipping dispatch. ` +
          `Wait for that run to finish, then tick the box again to re-run.`,
      );
      continue;
    }

    const inputs = {
      pr_number: String(prNumber),
      head_sha: headSha,
    };
    // >>> TEMP — REVERT BEFORE MERGE <<<
    // Dry-run: log what WOULD be dispatched instead of actually calling
    // createWorkflowDispatch. heavy_tests.yml isn't on `develop` yet so a
    // real dispatch would 404. This still exercises the regex / uncheck /
    // bot-loop guard / active-run check end-to-end. Restore the
    // createWorkflowDispatch try/catch block (see git history) before
    // merging.
    core.notice(
      `DRY-RUN: would dispatch ${workflow} (label: "${label}") ` +
        `with inputs ${JSON.stringify(inputs)}`,
    );
  }
  if (failed > 0) {
    core.setFailed(`${failed} of ${triggered.length} dispatch(es) failed.`);
  }
};
