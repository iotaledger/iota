Review a documentation page (or all pages changed in a PR) against project conventions.

## Checklist

For each page, verify:

### Diátaxis type

- [ ] The page has exactly one type tag: `tutorial`, `how-to`, `reference`, or `explanation`
- [ ] The content matches that type — no instructions in explanations, no "why" in how-tos, etc.
- [ ] The title follows the type convention (how-tos: imperative verb phrase; tutorials: "Build/Learn X")

### Frontmatter

- [ ] `description` is present and concise
- [ ] All tags exist in `content/tags.yml` — flag any that don't
- [ ] At least one technology tag is present alongside the Diátaxis tag

### Code

- [ ] No inline code blocks that duplicate source files — all code uses `file=<rootDir>/...` or the `reference` keyword
- [ ] `<rootDir>` paths resolve to real files under `docs/examples/` or elsewhere in `docs/`
- [ ] External `reference` URLs point to pinned versions (a tag or commit SHA, not `main`/`master`)

### Snippets and links

- [ ] No boilerplate that duplicates an existing snippet in `content/_snippets/`
- [ ] Internal links use doc IDs or relative paths, not absolute URLs

### Navigation

- [ ] The page is registered in the correct `content/sidebars/<section>.js` category
- [ ] No bare top-level string entries in the sidebar file
- [ ] If the page was moved or renamed, a redirect entry exists in `docusaurus.config.js` under `@docusaurus/plugin-client-redirects`

## Output format

Report as a checklist grouped by file. Flag failures with a brief explanation and a suggested fix. Mark passing items with ✓ so the overall status is clear at a glance.
