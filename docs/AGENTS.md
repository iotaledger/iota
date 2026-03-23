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

**Monorepo sources** — use `file=<rootDir>/...` with optional line range:

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

## Common Mistakes to Avoid

- **Mixing types:** A how-to that explains "why" is diluted; move the explanation to a dedicated explanation page and link to it.
- **Tutorial masquerading as how-to:** If the user needs no prior knowledge, it's a tutorial. If they already know what they want, it's a how-to.
- **Orphaned reference:** Auto-generated API docs alone are not enough. Every reference page benefits from how-to guides and explanation pages that give it context.
- **Explanation scope creep:** Explanation pages are the most likely to absorb stray instructions or reference tables. Keep them discursive and conceptual.
- **Inline code:** Never copy code into the page. Use `file=<rootDir>/...` for monorepo sources or the `reference` keyword with a GitHub URL for external repos. Both accept an `#L1-L10` line-range anchor.
