# Real-World How-To Pages — Style Guide

This file guides agents writing or editing real-world how-to pages in this directory. It supplements the parent `docs/CLAUDE.md` (Diataxis rules, code-embedding patterns, frontmatter requirements) and `docs/content/developer/iota-notarization/audit-trails/CLAUDE.md` (Audit Trails–specific conventions). Everything in those files applies here; this file adds patterns specific to real-world scenario pages.

## Purpose

Real-world pages are **how-to guides** that demonstrate a complete business scenario using the IOTA Audit Trails client packages. They show an experienced developer how to model a multi-party, role-scoped audit trail for a specific industry use case (e.g., customs clearance, clinical trials, digital product passports).

## Page structure

Every real-world how-to page follows this exact section order:

### 1. Frontmatter

```yaml
---
title: '<Scenario Name> - Audit Trails'
sidebar_label: '<Short Label>'
description: 'Real-world example demonstrating how to use IOTA Audit Trails to <one-line goal>.'
tags:
  - notarization
  - audit-trails
  - how-to
---
```

- `title` always ends with `- Audit Trails`.
- `sidebar_label` is a short form for the navigation sidebar.
- `description` starts with "Real-world example demonstrating how to use IOTA Audit Trails to…".
- Tags always include exactly `audit-trails` and `how-to`. No additional tags.

### 2. Imports

```mdx
import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';
```

Always import both, even if you think only one is needed — both are used for code tabs.

### 3. Title heading and introduction

```mdx
# <Same as frontmatter title>

This real-world example <one paragraph describing the scenario, the actors involved, and the key Audit Trails features demonstrated>.
```

The introduction is a single paragraph. It names the actors, summarises the workflow, and highlights the Audit Trails features in play (e.g., tag-scoped roles, write locking, time-constrained access).

### 4. Business Context

```mdx
## Business Context

<Industry context>. A blockchain-based audit trail provides:

- **<Benefit>**: <Explanation>
- **<Benefit>**: <Explanation>
- ...
```

- 3–5 bullet points.
- Each bullet starts with a **bold benefit label** followed by a colon and a one-sentence explanation.
- Focus on why a tamper-proof audit trail matters for this specific domain.

### 5. Field Usage Strategy

```mdx
## Field Usage Strategy

- **`immutable_metadata`**: <What identity data is stored>
- **`updatable_metadata`**: <What mutable status is tracked>
- **`record.data`**: <What event payloads contain>
- **`record.metadata`**: <What structured context looks like> (e.g., `"event:some_event"`)
- **`record.tag`**: <What categories are used> — list the tag names
```

Always cover all five fields in this exact order. Use inline code for field names and example values.

### 6. Role Design table

```mdx
### Role Design

| Role | Permissions | RoleTags | Holder |
|------|------------|----------|--------|
| `Admin` | Full administrative control | — | <who> |
| `<RoleName>` | <Permission list> | `"<tag>"` | <who> |
| ...  | ... | ... | ... |
```

- Always include an `Admin` row first.
- Use inline code for role names and tag strings.
- Use an em dash (`—`) when a column is not applicable (e.g., no RoleTags for Admin).
- The table may include an optional `Constraint` column if time-windowed or conditional access is relevant (see `clinical-trial.mdx`).

### 7. Prerequisites

```mdx
## Prerequisites

- A funded IOTA account
- Access to an IOTA network (testnet, devnet, or local)
- Audit Trails client packages installed
- Familiarity with [Role-Based Access Control](../explanations/role-based-access-control.mdx)<optional-extra-links>
```

Always include the first four bullets. Add links to additional explanation or how-to pages when the scenario uses features beyond basic RBAC (e.g., locking, tagged records).

### 8. Implementation Overview

```mdx
## Implementation Overview

### 1. <Step Title>

<One or two sentences describing what this step does and why.>

<CodeTabs />

### 2. <Step Title>

...
```

- Number each step sequentially (`### 1.`, `### 2.`, etc.).
- Step titles should be imperative verb phrases describing the goal (e.g., "Create the Trail", "Define Tag-Scoped Roles", "Lock the Trail After Clearance").
- Each step has a brief prose description followed by a code tab block.
- **3–7 steps** is the typical range. Fewer than 3 means the scenario is too simple for a real-world page; more than 7 means it should be split.

#### Code tab block pattern

Always use this exact structure for code tabs within implementation steps:

```mdx
<div className={'hide-code-block-extras'}>
<Tabs groupId="language" queryString>
<TabItem value="rust" label="Rust">

\`\`\`rust reference
https://github.com/iotaledger/notarization/tree/trails-v0.1/examples/audit-trail/real-world/<filename>.rs#L<start>-L<end>
\`\`\`

</TabItem>
<TabItem value="typescript-node" label="Typescript (Node.js)">

\`\`\`ts reference
https://github.com/iotaledger/notarization/tree/trails-v0.1/bindings/wasm/audit_trail_wasm/examples/src/real-world/<filename>.ts#L<start>-L<end>
\`\`\`

</TabItem>
</Tabs>
</div>
```

Key rules:

- Wrap in `<div className={'hide-code-block-extras'}>` to suppress extra UI.
- Use `groupId="language"` and `queryString` on `<Tabs>`.
- Tab values are always `rust` and `typescript-node`.
- Tab labels are always `Rust` and `Typescript (Node.js)`.
- Use `#L<start>-L<end>` line-range anchors for step snippets.
- Use the `reference` keyword — never copy code inline.

### 9. Real-World Applications

```mdx
## Real-World Applications

### <Application Name>
- **Scenario**: <One-sentence description>
- **Tags**: `"<tag1>"`, `"<tag2>"`, ...

### <Application Name>
...
```

- 3 related applications that show how the same pattern applies to adjacent domains.
- Each application has a `Scenario` line and a `Tags` line showing example tag names.

### 10. Running Examples Locally

```mdx
## Running Examples Locally

In order to run the examples, you will need to run an IOTA network locally. See the [local network setup](../getting-started/local-network-setup.mdx) guide.
```

Always include this section with the link to the local network setup guide.

### 11. Full Example Code

```mdx
## Full Example Code

<Tabs groupId="language" queryString>
<TabItem value="rust" label="Rust">

\`\`\`rust reference
https://github.com/iotaledger/notarization/tree/trails-v0.1/examples/audit-trail/real-world/<filename>.rs
\`\`\`

</TabItem>
<TabItem value="typescript-node" label="Typescript (Node.js)">

\`\`\`ts reference
https://github.com/iotaledger/notarization/tree/trails-v0.1/bindings/wasm/audit_trail_wasm/examples/src/real-world/<filename>.ts
\`\`\`

</TabItem>
</Tabs>
```

- **No** `<div className={'hide-code-block-extras'}>` wrapper — the full code section shows the complete UI chrome.
- No line-range anchor — embed the full file.
- This is always the last section on the page.

## Naming conventions

- File names use kebab-case: `customs-clearance.mdx`, `clinical-trial.mdx`.
- File names should be descriptive of the scenario, not the example number.
- Every new page must be registered in `docs/content/sidebars/notarization.js` under the `Real-World Examples` category.

## Checklist for new real-world pages

Before considering a page complete:

- [ ] Frontmatter has `title`, `sidebar_label`, `description`, and tags `[audit-trails, how-to]`.
- [ ] All 11 sections are present in the correct order.
- [ ] Business Context has 3–5 benefit bullets.
- [ ] Field Usage Strategy covers all five fields.
- [ ] Role Design table starts with Admin and lists all roles from the example.
- [ ] Implementation steps use the exact code tab block pattern with `hide-code-block-extras`.
- [ ] Full Example Code section does NOT use `hide-code-block-extras`.
- [ ] Both Rust and TypeScript tabs are present in every code block.
- [ ] All code references use the `reference` keyword with GitHub URLs — no inline code.
- [ ] Real-World Applications lists 3 related use cases.
- [ ] Running Examples Locally section links to the local network setup guide.
- [ ] Page is registered in `docs/content/sidebars/notarization.js`.
