# IOTA Audit Trail Documentation Style Guide

This file guides agents writing or editing pages under `docs/content/developer/iota-audit-trail/`. It supplements the parent `docs/CLAUDE.md` (Diataxis rules, code-embedding patterns, frontmatter requirements). Everything in the parent file applies here; this file adds product-specific conventions derived from the sibling `iota-notarization` documentation.

## Product context

IOTA Audit Trail provides tamper-proof, chronological records of activities on the IOTA ledger. It differs from IOTA Notarization: Notarization records *static facts* (a document existed at time T); Audit Trail records *sequences of events* (who did what, when). Audit Trail objects are **shared** on-chain and use **Role-Based Access Control (RBAC)** with Roles, Capabilities, and Record Tags.

The external source repository is **`https://github.com/iotaledger/notarization`**. The current tag is **`v0.1`**. Use this when constructing `reference` code-block URLs.

## Directory layout

Follow this structure exactly. Create missing folders as needed.

```
iota-audit-trail/
├── CLAUDE.md              # This file
├── index.mdx              # Product landing / introduction page
├── contribute.mdx         # How to contribute (repo links, Discord channel)
├── getting-started/       # Setup and installation guides
│   ├── rust.mdx
│   ├── wasm.mdx
│   └── local-network-setup.mdx
├── explanations/          # Conceptual deep-dives
├── how-tos/               # Goal-oriented step-by-step guides
│   ├── real-world/        # End-to-end scenario guides
│   └── ...                # Feature-specific guides
└── references/            # API docs (auto-generated Wasm, external Rust link)
    └── wasm/
```

## Sidebar

The sidebar is defined in `docs/content/sidebars/audit-trail.js`. Every new page must be added there. Follow the existing category hierarchy (Getting Started, Explanations, How To, References, Contribute). Keep the sidebar order aligned with the recommended reading path: introduction first, then getting-started, explanations, how-tos, references, contribute last.

## Tags

Use tags registered in `docs/content/tags.yml`. Every page must include:
1. Exactly one **Diataxis type tag**: `explanation`, `how-to`, `reference`, or `tutorial`.
2. The **product tag**: `audit-trail`.
3. Optional feature or technology tags (e.g., `rust`, `wasm`, `getting-started`).

If you need a new tag, add it to `tags.yml` under the `# Audit Trail` section.

## Frontmatter template

```yaml
---
title: '<Page title>'
description: '<One-line summary for SEO and link previews>'
sidebar_label: '<Short label for the sidebar, if different from title>'
tags:
  - <diataxis-type>   # one of: explanation, how-to, reference, tutorial
  - audit-trail
  - <optional-extra>
---
```

The `teams` field (e.g., `teams: [iotaledger/identity]`) is optional. Include it when the page is owned by a specific GitHub team.

## Page patterns

### index.mdx (Introduction / Landing page)

The introduction page is the product's front door. Pattern:

1. Frontmatter with `sidebar_label: Introduction` and tags `[reference, audit-trail]`.
2. Banner image: `![IOTA Audit Trail](/img/banner/banner_notarization.png)` (or a dedicated banner if available).
3. One-paragraph product summary.
4. Subsections covering: what the product solves, key use cases (with `:::info` admonitions for highlights), comparison to related products (e.g., Audit Trail vs. Dynamic Notarization), why IOTA, key actors, and a brief mention of RBAC linking to the explanation page.
5. No code on this page. Link out to getting-started and explanation pages instead.

### Explanation pages

Purpose: help the reader *understand* a concept. No step-by-step instructions.

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
- **Local Network Setup page**: start local chain, configure CLI, request faucet funds, publish the Audit Trail package, set `IOTA_AUDIT_TRAIL_PKG_ID` env var.

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
https://github.com/iotaledger/notarization/tree/feat/audit-trails-dev-examples-and-docs/examples/audit-trail/example_name.rs#L20-L32
\`\`\`

</TabItem>
<TabItem value="typescript-node" label="Typescript (Node.js)">

\`\`\`ts reference
https://github.com/iotaledger/notarization/tree/feat/audit-trails-dev-examples-and-docs/bindings/wasm/audit_trail_wasm/examples/src/example_name.ts#L20-L32
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

Place in `how-tos/real-world/`. These are longer how-to guides that demonstrate a complete business scenario (e.g., product passport, supply chain tracking). They follow the same structure as regular how-tos but include additional sections:
- **Business Context**: why this scenario matters.
- **Field Usage Strategy**: how the product's data model maps to the scenario.
- **Real-World Applications**: list of related use cases.

### Reference pages

API reference for Wasm is auto-generated and placed in `references/wasm/`. The Rust API reference is an external link to `https://iotaledger.github.io/notarization/audit_trail/index.html`. Do not manually author reference pages — they are generated from the source repository.

### contribute.mdx

Follow the notarization contribute page as a template. Update:
- Repository URL: `https://github.com/iotaledger/notarization`
- Discord channel: `#notarization-dev` (or the correct channel name)

## Writing style

- **Audience**: developers integrating Audit Trail into their applications. Assume familiarity with IOTA basics and blockchain concepts.
- **Tone**: technical, precise, direct. Avoid marketing language in explanation and how-to pages. The index page may use more persuasive language for use-case descriptions.
- Use `:::info`, `:::tip`, and `:::warning` admonitions sparingly and only when the information genuinely warrants callout treatment.
- Prefer "Audit Trail" (capitalized, two words) when referring to the product. Use lowercase "audit trail" only when referring to the generic concept.
- When referencing Move structs or types, use inline code: `AuditTrail`, `Capability`, `RoleMap`.
- Use **bold** for introducing key terms on first use in a page.
- Keep paragraphs short (3-5 sentences max). Use bullet lists for enumerations.

## Cross-referencing between products

When comparing Audit Trail to Notarization, link to the notarization docs with relative paths: `../../iota-notarization/explanations/dynamic-notarization.mdx`. Do not duplicate notarization content — summarize the distinction and link out.

## Checklist for new pages

Before considering a page complete:

- [ ] Frontmatter includes `description`, at least one Diataxis type tag, and `audit-trail`.
- [ ] Page is added to `docs/content/sidebars/audit-trail.js`.
- [ ] Any new tags are registered in `docs/content/tags.yml`.
- [ ] Code blocks use the `reference` keyword with GitHub URLs (no inline code copies).
- [ ] Both Rust and TypeScript tabs are present in how-to guides.
- [ ] Relative cross-links work (no broken paths).
- [ ] The page stays pure to its Diataxis type (no how-to steps in an explanation, no explanations in a how-to).