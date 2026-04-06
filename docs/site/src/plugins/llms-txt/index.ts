// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import fs from 'fs';
import path from 'path';
import type { LoadContext, Plugin } from '@docusaurus/types';

export default function llmsTxtPlugin(context: LoadContext): Plugin {
    const { siteConfig } = context;
    const { title, tagline } = siteConfig;

    const content = `\
# ${title}

> ${tagline}

## About IOTA

- [About IOTA](https://docs.iota.org/llms-full-about.txt): Architecture, tokenomics, and programs

## Developers

- [Getting Started](https://docs.iota.org/llms-full-developer-getting-started.txt): Installation, environment setup, and first steps
- [Explanations](https://docs.iota.org/llms-full-developer-explanations.txt): Cryptography and transaction authentication concepts
- [How To](https://docs.iota.org/llms-full-developer-how-to.txt): Transactions, sponsored transactions, PTBs, and exchange integration
- [Tutorials](https://docs.iota.org/llms-full-developer-tutorials.txt): Step-by-step tutorials for building on IOTA
- [Move](https://docs.iota.org/llms-full-developer-move.txt): Object model, standards, patterns, and framework references
- [SDKs](https://docs.iota.org/llms-full-developer-sdks.txt): TypeScript and Rust SDK documentation
- [GraphQL](https://docs.iota.org/llms-full-developer-graphql.txt): GraphQL API guides and references
- [CLI](https://docs.iota.org/llms-full-developer-cli.txt): IOTA CLI reference documentation
- [Trust Framework](https://docs.iota.org/llms-full-developer-trust-framework.txt): IOTA Identity, Notarization, and Hierarchies
- [Stardust Migration](https://docs.iota.org/llms-full-developer-stardust.txt): Migration guides from IOTA Stardust
- [References](https://docs.iota.org/llms-full-developer-references.txt): API references, glossary, and contribution guides

## Operators

- [Operators](https://docs.iota.org/llms-full-operator.txt): Full nodes, validators, and infrastructure

## Users

- [Users](https://docs.iota.org/llms-full-users.txt): Wallets and IOTA applications

## Workshops

- [Workshops](https://docs.iota.org/llms-full-workshops.txt): Hands-on workshop materials

## Optional

- [Full Documentation](https://docs.iota.org/llms-full.txt): Complete documentation combined in a single file
`;

    return {
        name: 'llms-txt',
        async postBuild({ outDir }: { outDir: string }): Promise<void> {
            fs.writeFileSync(path.join(outDir, 'llms.txt'), content);
        },
    };
}
