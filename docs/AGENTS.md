# Documentation Agents Guide

This file guides agents writing or reviewing documentation in this directory. All docs follow the [Diátaxis framework](https://diataxis.fr/). Understanding which type a page belongs to — and keeping it pure — is the most important rule.

## The Four Documentation Types

Diátaxis organizes docs on two axes: **action vs. cognition** and **acquisition vs. application**. This produces four distinct types. Mixing types degrades all of them.

| Type         | User's question                 | User state               | Axis                  |
| ------------ | ------------------------------- | ------------------------ | --------------------- |
| Tutorial     | "Teach me how to do this"       | New learner              | Action + acquiring    |
| How-to guide | "How do I achieve X?"           | Experienced, goal-driven | Action + applying     |
| Reference    | "What is the exact spec for Y?" | Working practitioner     | Cognition + applying  |
| Explanation  | "Why does this work this way?"  | Reflective practitioner  | Cognition + acquiring |

### Tutorials

A tutorial guides a learner through acquiring a skill via hands-on activity. The goal is **skill and confidence**, not task completion.

**Rules:**

- Minimize explanation — link out to explanation pages instead
- Deliver rapid, visible feedback at every step so learners see cause and effect
- Stay concrete; guide through specific actions, never abstract concepts
- Every step must work for every user — reliability is non-negotiable
- Never offer alternatives or choices that distract from the guided path

**Avoid:** Teaching via explanation, offering options mid-guide, using abstract language, and assuming learners will notice important details on their own.

**Frontmatter tags:** `tutorial`

### How-to Guides

A how-to guide directs an **already capable user** through achieving a specific goal. It is a contract: "if you face this situation, follow these steps."

**Rules:**

- Focus strictly on action — no digressions, no teaching moments, no embedded explanations
- Address real-world goals, not tool mechanics
- Sequence steps logically in the order the user thinks and works
- Be flexible enough for users to adapt to their specific circumstances
- Use precise, descriptive titles: `Create an authenticator function`, not `Authenticators`

**Avoid:** Teaching concepts the user should already know, inserting reference info or context that interrupts guidance, conflating with tutorials.

**Frontmatter tags:** `how-to`

#### Code embedding

Never copy code inline. Always reference the source so docs stay in sync with the implementation. There are two patterns depending on where the code lives:

**Monorepo sources** — use `file=<rootDir>/...` with optional line range. `<rootDir>` resolves to the `docs/` directory:

````mdx
```move file=<rootDir>/examples/move/my-module/sources/foo.move#L10-L25
```
````

**External repositories** — use the `reference` keyword with a GitHub URL and optional anchor:

````mdx
```rust reference
https://github.com/iotaledger/notarization/tree/v0.1/examples/02_create.rs#L20-L32
```
````

Both patterns accept a bare file path/URL (no line range) to embed the full file.

#### How-to guide structure (follow this pattern)

Based on the existing guides in `content/developer/move/how-tos/`:

````mdx
---
description: 'How to <do specific thing>'
tags:
  - how-to
  - <relevant-technology-tag>
---

# <Imperative title: "Create X" / "Enforce Y" / "Configure Z">

<OptionalContextSnippet />

This how-to demonstrates how to <concise one-line goal>.

## Example Code

1. <Step description.>
```move file=<rootDir>/examples/move/my-module/sources/foo.move#L1-L10
```
2. <Step description.>
```move file=<rootDir>/examples/move/my-module/sources/foo.move#L20-L30
```

## Expected Behavior  <!-- optional: include when the outcome isn't self-evident -->

- <Observable outcome 1>
- <Observable outcome 2>

## Full Example Code

```move file=<rootDir>/examples/move/my-module/sources/foo.move
```
````

### Reference

Reference is technical description users consult **during** their work. It describes the machinery objectively.

**Rules:**

- Describe, don't instruct — no "do this", only "this is"
- Be authoritative and precise to eliminate all doubt
- Mirror the product's own structure (e.g., one page per module/type/function)
- Design for lookup, not narrative reading — consistent formatting matters more than prose quality
- Short illustrative examples are fine; do not drift into tutorial territory

**Avoid:** Opinion, interpretation, instructional tone, narrative flow.

**Frontmatter tags:** `reference`

### Explanation

Explanation provides context, design decisions, and deeper understanding. It is the only type that makes sense to read away from the product.

**Rules:**

- Make connections across topics and to broader ecosystem context
- Provide context: design decisions, tradeoffs, constraints, history
- Discuss alternatives and the reasoning behind choices
- Admit perspective — understanding always comes from a viewpoint
- Keep scope tight — explanation is the type most prone to absorbing content that belongs elsewhere

**Avoid:** Instructions, reference tables, step-by-step sequences — these belong in the other three types.

**Frontmatter tags:** `explanation`

## Before You Write

1. **Search for duplicates** — `grep -r "your topic" content/` before creating a new page.
2. **Check `_snippets/`** — reusable MDX for common warnings, install steps, faucet info, network resets, and more. Import with a relative path and use as a JSX component:
   ```mdx
   import NetworkReset from '../../../_snippets/network-reset.mdx';
   <NetworkReset />
   ```
3. **Pick the Diátaxis type** — if unsure, re-read the type rules above; choose based on the user's _state_, not the content's subject.
4. **Locate the sidebar file** — find `content/sidebars/<section>.js` before writing so you know where to register the page.

## File Organization

```
docs/content/
├── developer/
│   └── <topic>/
│       ├── how-tos/          # How-to guides
│       ├── tutorials/        # Tutorials
│       ├── explanations/     # Explanation pages
│       └── references/       # Reference pages
└── _snippets/                # Reusable MDX snippets (not standalone pages)
```

Place new pages in the correct subfolder by type. If no subfolder exists for the type you need, create it.

## Frontmatter Requirements

Every page needs at least:

```yaml
---
description: '<Concise description of what this page covers>'
tags:
  - <type-tag>        # one of: tutorial, how-to, reference, explanation
  - <technology-tag>  # e.g. move-sc, move-vm, typescript, graphql
---
```

## Navigation / Sidebar Config

New pages are **not** auto-discovered — you must register them in the correct sidebar file under `content/sidebars/`. Choose the file that matches the section (`developer.js`, `operator.js`, `users.js`, etc.).

Sidebar files are deeply nested `type: 'category'` trees. Find the right category and add the page's path (relative to `content/`, no extension) inside its `items` array:

```js
{
    type: 'category',
    label: 'How-tos',
    items: [
        'developer/move/how-tos/existing-page',
        'developer/move/how-tos/create-authenticator', // ← add here
    ],
},
```

Never append a bare string to the top of the file — it will build but not appear in the navigation. `site/sidebars.js` is the top-level entry point that imports all section files; check it only when adding an entirely new section.

## Valid Tags

Tags must exist in `content/tags.yml` — never invent a tag. If you need a new one, add it to `tags.yml` first.

**Diátaxis (required — exactly one per page):** `tutorial`, `how-to`, `reference`, `explanation`

**Languages:** `move-sc`, `typescript`, `rust`, `python`, `solidity`, `go`, `wasm`, `kotlin`, `swift`

**VMs:** `move-vm`, `evm`

**Tooling:** `sdk`, `ts-sdk`, `rust-sdk`, `cli`, `iota-cli`, `dapp-kit`, `crates`

**Networks:** `devnet`, `testnet`, `mainnet`, `localnet`

**Concepts:** `transaction`, `nft`, `native-token`, `address`, `consensus`, `randomness`, `faucet`

See `content/tags.yml` for the full list grouped by category.

## Build Pipeline

Both `dev` and `build` run the same preparation steps before starting Docusaurus:

```
download-rpc-specs          # JSON-RPC / OpenRPC specs
download-iota-references    # Move framework refs  ──┐
download-iota-sdk-references # Rust SDK refs         │  pre-built tarballs
download-external-references # EVM / Identity /      │  from AWS S3
                             # Notarization /         │  (files.iota.org)
                             # Hierarchies refs      ─┘
generate-ts-docs            # TypeDoc from TS SDK source (local)
generate-graphql-docs       # GraphQL schema → docs, per network (local)
gen-api                     # OpenAPI / REST API docs (local)
```

**Do not hand-edit files under these paths** — they are overwritten on every build:

- `content/developer/references/framework/`
- `content/developer/ts-sdk/`
- `content/developer/iota-sdk/references/`
- `content/developer/iota-evm/references/`
- `content/developer/iota-identity/references/`
- `content/developer/iota-notarization/single-notarization/references/`
- `content/developer/iota-notarization/audit-trails/references/`
- `content/developer/iota-hierarchies/references/`

If a generated reference is wrong, fix the source (the relevant SDK repo or script in `site/scripts/`) rather than the output file.

## Running the Docs

Always run from the **repo root** after `pnpm i`:

```sh
pnpm iota-docs dev    # start dev server (hot-reload, localhost:3000)
pnpm iota-docs build  # production build — use this to validate before merging
```

Both commands run the full preparation pipeline (downloads + code generation) before starting Docusaurus.

To run a specific docs script without the full build, use the pnpm workspace filter:

```sh
pnpm --filter iota-docs run generate-ts-docs
pnpm --filter iota-docs run download-iota-references
```

`onBrokenLinks`, `onBrokenMarkdownLinks`, and `onBrokenAnchors` are all set to `throw`, so any broken reference fails the build.

## Notable Plugins

Three plugins directly affect how you write content:

### rehype-jargon

Domain terms defined in `site/config/jargon.js` are automatically highlighted in rendered pages with a tooltip showing the definition. To trigger a tooltip, wrap the term in underscores in your MDX: `_gas_`, `_epoch_`, `_finality_`. To add a new term, add an entry to `jargon.js` — keys are lowercase, values are HTML strings.

### plugin-client-redirects

When you rename or move a page, add a redirect so external links and search engine results don't break. Redirects live in `docusaurus.config.js` inside the `@docusaurus/plugin-client-redirects` config:

```js
{ from: '/old/path', to: '/new/path' },
```

The redirect logic derives child paths automatically, so a single entry covers `/old/path/sub-page` → `/new/path/sub-page`.

Redirects are a grace period, not permanent. Once external links and search indexes have had reasonable time to update (a few months), remove the entry to keep the config clean.

### docusaurus-plugin-llms

Generates `llms-full-*.txt` files consumed by AI tools and agents. Each file covers a topic area via `includePatterns`. When adding a significant new content section, add it to the relevant pattern in `docusaurus.config.js`, or create a new `customLLMFiles` entry if it forms a distinct topic. Snippets (`_snippets/`) and files starting with `_` are excluded automatically.

## Common Mistakes to Avoid

- **Mixing types:** A how-to that explains "why" is diluted; move the explanation to a dedicated explanation page and link to it.
- **Tutorial masquerading as how-to:** If the user needs no prior knowledge, it's a tutorial. If they already know what they want, it's a how-to.
- **Orphaned reference:** Auto-generated API docs alone are not enough. Every reference page benefits from how-to guides and explanation pages that give it context.
- **Explanation scope creep:** Explanation pages are the most likely to absorb stray instructions or reference tables. Keep them discursive and conceptual.
- **Inline code:** Never copy code into the page. Use `file=<rootDir>/...` for monorepo sources or the `reference` keyword with a GitHub URL for external repos. Both accept an `#L1-L10` line-range anchor.
- **Unregistered page:** A new page not added to its sidebar file will build but never appear in the navigation.
- **Missing redirect:** Renaming or moving a page without a redirect entry in `docusaurus.config.js` breaks existing links silently — the build will succeed but users will hit 404s.
