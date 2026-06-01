# IOTA Audit Trails Documentation Style Guide

This file guides agents writing or editing pages under `docs/content/developer/iota-notarization/`.
It supplements the parent `docs/CLAUDE.md` (Diataxis rules, code-embedding patterns, frontmatter requirements).
Everything in the parent file applies here; this file adds product-specific conventions derived from the sibling `iota-notarization` documentation.

## Product context

The IOTA Notarization Toolkit, a set of IOTA ledger tools for verifiable on-chain data workflows and consists of the
**Single Notarization** and **Audit Trails** components as been described in the
[external source repository Main Readme](https://github.com/iotaledger/notarization/blob/audit-trails-dev/README.md)

The external source repository is **`https://github.com/iotaledger/notarization`**. The current tag is **`v0.1`**. 
Use this when constructing `reference` code-block URLs.

The external source repository also provides a `Naming Conventions` section in the 
[root `CLAUDE.md` file](https://github.com/iotaledger/notarization/blob/audit-trails-dev/CLAUDE.md) which can be seen
as the source of truth regarding wording, terminology, prose and capitalization rules.

### Single Notarization Component

Single Notarization provides two Notarization Methods for creating verifiable, on-chain records of any individual piece
of digital data by anchoring data to the IOTA ledger: 
-   **Locked Notarization**: For creating permanent, static records that can not be changed until the object is destroyed.
-   **Dynamic Notarization**: For creating records of evolving data where only the most current version is relevant.


### Audit Trails Component

IOTA Audit Trails provides tamper-proof, chronological records of activities on the IOTA ledger. 
It differs from IOTA Notarization: Notarization records _static facts_ (a document existed at time T); 
Audit Trails records _sequences of events_ (who did what, when). 
`AuditTrail` objects are **shared** on-chain and use **Role-Based Access Control (RBAC)** with Roles, Capabilities, and Record Tags.

## Directory layout

Follow this structure exactly. Create missing folders as needed.

```
iota-notarization
├── CLAUDE.md                   # This file
├── index.mdx                   # Toolkit landing / IOTA Notarization Toolkit introduction
├── contribute.mdx
├── audit-trail/
│   ├── index.mdx               # Component landing / Audit Trails introduction page
│   ├── getting-started/        # Setup and installation guides
│   │   ├── rust.mdx
│   │   ├── wasm.mdx
│   │   └── local-network-setup.mdx
│   ├── explanations/    │      # Conceptual deep-dives
│   ├── how-tos/                # Goal-oriented step-by-step guides
│   ├── real-world-examples/    # End-to-end scenario guides
│   └── references/             # API docs (auto-generated Wasm, external Rust link)
│       └── wasm/
├── single-notarization/
│   ├── index.mdx               # Component landing / Single Notarization introduction page
│   ...                         # Same directory as been used for Audit Trails 
```

## Sidebar

The sidebar is defined in `docs/content/sidebars/notarization.js` (the unified Notarization sidebar that covers both Single Notarization and Audit Trails).
Every new page must be added there under the `Audit Trails` or `Single Notarization` category.
Keep the sidebar order aligned with the recommended reading path: introduction first, then getting-started, explanations, how-tos, references.

## Tags

Use tags registered in `docs/content/tags.yml`. Every page must include:

1. Exactly one **Diataxis type tag**: `explanation`, `how-to`, `reference`, or `tutorial`.
2. The **product tag**: `notarization`.
3. A **component tag**: `audit-trails` or `single-notarization`.
4. Optional feature or technology tags (e.g., `rust`, `wasm`, `getting-started`).

If you need a new tag, add it to `tags.yml` under the `# Notarization` section.

## Frontmatter template

```yaml
---
title: '<Page title>'
description: '<One-line summary for SEO and link previews>'
sidebar_label: '<Short label for the sidebar, if different from title>'
tags:
  - <diataxis-type>   # one of: explanation, how-to, reference, tutorial
  - notarization      # product tag
  - <component tag>   # `audit-trails` or `single-notarization`
  - <optional-extra>
---
```

The `teams` field (e.g., `teams: [iotaledger/identity]`) is optional. Include it when the page is owned by a specific GitHub team.

## Page patterns

### index.mdx (Introduction / Landing page)

The introduction page is the product's front door. Pattern:

1. Frontmatter with `sidebar_label: Introduction` and tags `[reference, notarization, component tag>]`.
2. Banner image: Following banners exist:
   - `![IOTA Notarization Toolkit](/img/banner/banner_notarization.png)`
   - `![Single Notarization](/img/banner/banner_single_notarization.png)`
   - `![Audit Trails](/img/banner/banner_audit_trail.png)`
3. One-paragraph product summary.
4. Subsections covering: what the product solves, key use cases (with `:::info` admonitions for highlights), comparison to 
   related products (e.g., Audit Trails vs. Dynamic Notarization), why IOTA, key actors, and a brief mention of RBAC linking to the explanation page.
5. No code on this page. Link out to getting-started and explanation pages instead.

### Explanation pages

Purpose: help the reader _understand_ a concept. No step-by-step instructions.

- **One concept per page.** Examples: "The Audit Trail Object", "Role-Based Access Control", "Record Tags and Permissions".
- Use horizontal rules (`---`) to separate major sections.
- Bold the first mention of a key term (e.g., **Capability**, **RoleMap**).
- Use tables for structured comparisons (feature matrices, permission sets, validation rules).
- Cross-link related explanation pages with relative paths: `[Role-Based Access Control](./role-based-access-control.mdx)`.
- Inline code blocks for Move structs or enums are acceptable to illustrate data structures. Use plain `rust` fenced blocks (not the `reference` keyword) for illustrative snippets that are not runnable examples.
- Rust API quick-reference snippets are acceptable in explanation pages when they clarify a concept (see the RBAC page as precedent). Keep them short and illustrative.
- End each page with a clear "what to read next" direction, either through cross-links or a Related section.

### Getting-started pages

Purpose: get the developer from zero to a working setup.

- **Rust page**: requirements, Cargo dependency, clone + build + run example.
- **Wasm page**: Node.js requirements, npm install, Node.js vs. Web imports, usage example with tabs, link to API reference.
- **Local Network Setup page**: start local chain, configure CLI, request faucet funds, publish the Move package, set needed env var.

Each page should be self-contained. A developer following only that page should be able to run their first example.

### How-to guides

Purpose: direct an experienced user through a specific goal.

Structure (follow exactly):

1. Title: imperative verb phrase ("Create a Trail", "Add a Record", "Revoke a Capability").
2. Brief one-line goal statement.
3. Prerequisites (bulleted list).
4. Numbered steps, each with a code block using language tabs.
5. Optional "End Result" or "Expected Behavior" section.
6. "Full Example Code" section with complete runnable file.
7. "Running Examples Locally" note at the bottom.

#### Code tabs

Always provide **Rust** and **TypeScript (Node.js)** tabs using this pattern:

```mdx
<div className={'hide-code-block-extras'}>
<Tabs groupId="language" queryString>
<TabItem value="rust" label="Rust">

\`\`\`rust reference
https://github.com/iotaledger/notarization/tree/feat/audit-trails-dev/examples/audit-trail/example_name.rs#L20-L32
\`\`\`

</TabItem>
<TabItem value="typescript-node" label="Typescript (Node.js)">

\`\`\`ts reference
https://github.com/iotaledger/notarization/tree/feat/audit-trails-dev/bindings/wasm/audit_trail_wasm/examples/src/example_name.ts#L20-L32
\`\`\`

</TabItem>
</Tabs>
</div>
```

Key rules:

- Use `groupId="language"` and `queryString` on every `<Tabs>` so the user's language choice persists across pages.
- Use the `reference` keyword with GitHub URLs for all code — never copy code inline (see parent CLAUDE.md).
- Wrap tab blocks in `<div className={'hide-code-block-extras'}>` to suppress extra UI chrome, except for the "Full Example Code" section at the bottom.
- Include `#L<start>-L<end>` line-range anchors for step-specific snippets. Omit the anchor for full-file embeds.

#### Real-world examples

Place in `real-world-examples/`. These are longer how-to guides that demonstrate a complete business scenario (e.g., product passport, supply chain tracking). 
They follow the same structure as regular how-tos but include additional sections:

- **Business Context**: why this scenario matters.
- **Field Usage Strategy**: how the product's data model maps to the scenario.
- **Real-World Applications**: list of related use cases.

### Reference pages

API reference for Wasm is auto-generated and placed in `references/wasm/`.
The Rust API reference is an external link to 
- Single Notarization: `https://iotaledger.github.io/notarization/notarization/index.html`
- Audit Trails: `https://iotaledger.github.io/notarization/audit_trail/index.html`

Do not manually author reference pages — they are generated from the source repository.

## Writing style

- **Audience**: developers integrating Audit Trails into their applications. Assume familiarity with IOTA basics and blockchain concepts.
- **Tone**: technical, precise, direct. Avoid marketing language in explanation and how-to pages. The index page may use more persuasive language for use-case descriptions.
- Use `:::info`, `:::tip`, and `:::warning` admonitions sparingly and only when the information genuinely warrants callout treatment.
- **Product naming** (the [`Naming Conventions` in the notarization repo `CLAUDE.md`](https://github.com/iotaledger/notarization/blob/audit-trails-dev/CLAUDE.md) is the source of truth):
  - Use **"Audit Trails"** (plural, title case) when referring to the **product / component / package / client** — e.g. "Audit Trails provides …", "the Audit Trails package", "IOTA Audit Trails".
  - For a **single on-chain object**: use **"Audit Trail"** (singular, title case) in titles, headings, and general descriptive prose; use **"`AuditTrail` object"** (the Move type in backticks) in normal paragraphs that describe creating, deleting, updating, configuring, or otherwise directly interacting with the object (e.g. "creating a new `AuditTrail` object", "remove an `AuditTrail` object from the network").
  - For **multiple instances**, use lowercase plural **"audit trails"** (except at the start of a sentence or in a heading).
  - Use lowercase **"audit trail"** only for the generic, non-product concept (e.g. "an audit trail is a sequential log").
- When referencing Move structs or types, use inline code: `AuditTrail`, `Capability`, `RoleMap`.
- Use **bold** for introducing key terms on first use in a page.
- Keep paragraphs short (3-5 sentences max). Use bullet lists for enumerations.

## Cross-referencing between products

When comparing Audit Trails to Single Notarization, link to the sibling docs with relative paths:
`../../single-notarization/explanations/dynamic-notarization.mdx`. Do not duplicate notarization content — summarize the distinction and link out.

## Keeping example-code line references up to date

How-to guides embed snippets from the [notarization repository](https://github.com/iotaledger/notarization) using `#L<start>-L<end>` anchors. 
When example code in that repository changes, those anchors go stale. The script `scripts/update_doc_refs.py` automates the remapping.

### When to run it

Run the script after any commit in `iotaledger/notarization` that modifies files under:

- `examples/audit-trail/`  (Rust examples for Audit Trails)
- `examples/notarization/` (Rust examples for Single Notarization)
- `bindings/wasm/audit_trail_wasm/examples/` (TypeScript examples for Audit Trails)
- `bindings/wasm/notarization_wasm/examples/` (TypeScript examples for Single Notarization)
- `examples/utils/utils.rs`

### Prerequisites

- Python 3.10+
- A local clone of `https://github.com/iotaledger/notarization` with the relevant branch fetched.

### Usage

```sh
python3 docs/content/developer/iota-notarization/scripts/update_doc_refs.py \
    --notarization-repo /path/to/notarization/repo-clone  \
    --old-ref <commit-or-ref-the-docs-currently-target> \
    --new-ref <commit-or-branch-to-update-to>
```

All flags can also be set via environment variables, which is convenient for CI or repeated use:

| Flag | Env var | Description                                              |
|------|---------|----------------------------------------------------------|
| `--notarization-repo` | `NOTARIZATION_REPO` | Path to the local notarization repo clone                |
| `--docs-dir` | `DOCS_DIR` | Path to the `how-tos/` folder (auto-detected by default) |
| `--old-ref` | `OLD_REF` | Git ref the docs currently reference                     |
| `--new-ref` | `NEW_REF` | Git ref to update the docs to                            |
| `--dry-run` | — | Print changes without writing any files                  |

### Example workflow

1. Identify the commit that changed the example files — for instance `b704131` on branch `feat/audit-trails-dev`.
2. Find the last commit the docs were synchronized to (the parent of that commit, or the tag/ref recorded in the previous update PR).
3. Run the script in dry-run mode first to review the changes:

   ```sh
   python3 docs/content/developer/iota-notarization/audit-trail/scripts/update_doc_refs.py \
       --notarization-repo ~/code/notarization \
       --old-ref 34190c6 \
       --new-ref origin/feat/audit-trails-dev \
       --dry-run
   ```

4. Remove `--dry-run` to apply the changes, then commit the updated MDX files.

### How it works

The script uses Python's `difflib.SequenceMatcher` to diff each example file between the two refs and builds a line-number mapping. 
For each `#L<start>-L<end>` anchor found in the MDX files, it looks up both boundary lines in that mapping and replaces the anchor with the new range.
Lines unchanged between the two refs map exactly; replaced or moved lines map to their best counterpart in the new file.

## Checklist for new pages

Before considering a page complete:

- [ ] Frontmatter includes `description`, at least one Diataxis type tag, `notarization` and a <component tag>.
- [ ] Page is added to `docs/content/sidebars/notarization.js` (under the `Audit Trails` or `Single Notarization` category).
- [ ] Any new tags are registered in `docs/content/tags.yml`.
- [ ] Code blocks use the `reference` keyword with GitHub URLs (no inline code copies).
- [ ] Both Rust and TypeScript tabs are present in how-to guides.
- [ ] Relative cross-links work (no broken paths).
- [ ] The page stays pure to its Diataxis type (no how-to steps in an explanation, no explanations in a how-to).
