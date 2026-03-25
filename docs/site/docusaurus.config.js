// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { themes } from "prism-react-renderer";
import path from "path";
import math from "remark-math";
import katex from "rehype-katex";
import codeImport from "remark-code-import";

require("dotenv").config();

const jargonConfig = require('./config/jargon.js');

const typedocBaseConfig = {
  skipErrorChecking: true,
  plugin: ['typedoc-plugin-markdown'],
  githubPages: false,
  readme: 'none',
  hideGenerator: true,
  sort: ['source-order'],
  excludeInternal: true,
  excludePrivate: true,
  excludeExternals: true,
  disableSources: true,
  hideBreadcrumbs: true,
  intentionallyNotExported: [],
  useCodeBlocks: true,
  parametersFormat: 'table',
  interfacePropertiesFormat: 'table',
  classPropertiesFormat: 'table',
  typeDeclarationFormat: 'table',
  enumMembersFormat: 'table',
  indexFormat: 'table',
  tableColumnSettings: {
    hideSources: true,
    leftAlignHeaders: true,
  },
};

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: "IOTA Documentation",
  tagline:
    "IOTA is a next-generation smart contract platform with high throughput, low latency, and an asset-oriented programming model powered by Move",
  favicon: "/icons/favicon.ico",
  url: "https://docs.iota.org",
  baseUrl: "/",
  
  customFields: {
    amplitudeKey: process.env.AMPLITUDE_KEY,
  },

  onBrokenLinks: "throw",
  onBrokenMarkdownLinks: "throw",
  onBrokenAnchors: "throw",

  future: {
    v4: {
      removeLegacyPostBuildHeadAttribute: true,
    },
    experimental_faster: true,
  },

  markdown: {
    format: "detect",
    mermaid: true,
  },
  plugins: [
    [
      'docusaurus-plugin-llms',
      {
        docsDir: '../content',
        pathTransformation: {
          ignorePaths: ['docs', '..', 'content']
        },
        // Ignore everything with an underscore which is docusaurus default behaviour
        ignoreFiles: [ '**/_**' ],
        // llms.txt is maintained by the llms-txt plugin at src/plugins/llms-txt
        generateLLMsTxt: false,
        customLLMFiles: [
          {
            filename: 'llms-full-about.txt',
            title: 'IOTA Documentation - About IOTA',
            description: 'Complete About IOTA documentation covering architecture, tokenomics, and programs',
            includePatterns: ['about-iota/**'],
            fullContent: true,
          },
          {
            filename: 'llms-full-operator.txt',
            title: 'IOTA Documentation - Operators',
            description: 'Complete operator documentation for running full nodes, validators, and infrastructure',
            includePatterns: ['operator/**'],
            fullContent: true,
          },
          {
            filename: 'llms-full-users.txt',
            title: 'IOTA Documentation - Users',
            description: 'Complete user documentation for wallets and IOTA applications',
            includePatterns: ['users/**'],
            fullContent: true,
          },
          {
            filename: 'llms-full-workshops.txt',
            title: 'IOTA Documentation - Workshops',
            description: 'Workshop materials for hands-on learning with IOTA',
            includePatterns: ['developer/workshops/**'],
            fullContent: true,
          },
          {
            filename: 'llms-full-developer-getting-started.txt',
            title: 'IOTA Developer Documentation - Getting Started',
            description: 'Getting started guides including installation, environment setup, and first steps with IOTA development',
            includePatterns: [
              'developer/developer.md',
              'developer/network-overview.md',
              'developer/getting-started/**',
            ],
            fullContent: true,
          },
          {
            filename: 'llms-full-developer-explanations.txt',
            title: 'IOTA Developer Documentation - Explanations',
            description: 'Conceptual explanations including cryptography, transaction authentication, and smart contract security',
            includePatterns: [
              'developer/cryptography/**',
            ],
            fullContent: true,
          },
          {
            filename: 'llms-full-developer-how-to.txt',
            title: 'IOTA Developer Documentation - How To',
            description: 'How-to guides for transactions, sponsored transactions, PTBs, and exchange integration',
            includePatterns: [
              'developer/iota-101/transactions/**',
              'developer/exchange-integration.md',
              'developer/advanced/custom-indexer.md',
            ],
            fullContent: true,
          },
          {
            filename: 'llms-full-developer-tutorials.txt',
            title: 'IOTA Developer Documentation - Tutorials',
            description: 'Step-by-step tutorials for building applications on IOTA',
            includePatterns: [
              'developer/tutorials/**',
            ],
            fullContent: true,
          },
          {
            filename: 'llms-full-developer-move.txt',
            title: 'IOTA Developer Documentation - Move',
            description: 'Move language documentation including the object model, standards, patterns, framework references, and challenges',
            includePatterns: [
              'developer/iota-101/objects/**',
              'developer/iota-101/move-overview/**',
              'developer/iota-101/create-coin/**',
              'developer/iota-101/nft/**',
              'developer/iota-101/using-events.md',
              'developer/iota-101/access-time.md',
              'developer/references/framework/**',
              'developer/references/iota-move.md',
              'developer/references/move/**',
              'developer/standards/**',
              'developer/advanced/introducing-move-2024.md',
              'developer/advanced/onchain-randomness.md',
              'developer/advanced/asset-tokenization.md',
              'developer/iota-move-ctf/**',
              'developer/evm-to-move/**',
            ],
            fullContent: true,
          },
          {
            filename: 'llms-full-developer-sdks.txt',
            title: 'IOTA Developer Documentation - SDKs',
            description: 'TypeScript and Rust SDK documentation for building applications on IOTA',
            includePatterns: [
              'developer/ts-sdk/**',
              'developer/references/rust-sdk.md',
            ],
            fullContent: true,
          },
          {
            filename: 'llms-full-developer-graphql.txt',
            title: 'IOTA Developer Documentation - GraphQL',
            description: 'GraphQL API how-to guides and reference documentation',
            includePatterns: [
              'developer/getting-started/graphql-rpc.md',
              'developer/graphql-rpc.md',
              'developer/advanced/graphql-migration.md',
              'developer/references/iota-graphql.md',
              'developer/references/iota-api/iota-graphql/**',
            ],
            fullContent: true,
          },
          {
            filename: 'llms-full-developer-cli.txt',
            title: 'IOTA Developer Documentation - CLI',
            description: 'Complete CLI reference for the IOTA command-line interface',
            includePatterns: [
              'developer/references/cli.md',
              'developer/references/cli/**',
            ],
            fullContent: true,
          },
          {
            filename: 'llms-full-developer-trust-framework.txt',
            title: 'IOTA Developer Documentation - IOTA Trust Framework',
            description: 'IOTA Trust Framework documentation including Identity, Notarization, and Hierarchies',
            includePatterns: [
              'developer/iota-trust-framework.md',
              'developer/iota-identity/**',
              'developer/iota-notarization/**',
              'developer/iota-hierarchies/**',
            ],
            fullContent: true,
          },
          {
            filename: 'llms-full-developer-stardust.txt',
            title: 'IOTA Developer Documentation - Migrating from Stardust',
            description: 'Migration guides and documentation for transitioning from IOTA Stardust to the new IOTA network',
            includePatterns: [
              'developer/stardust/**',
              'developer/dev-cheat-sheet.md',
            ],
            fullContent: true,
          },
          {
            filename: 'llms-full-developer-references.txt',
            title: 'IOTA Developer Documentation - References',
            description: 'API references, execution architecture, research papers, glossary, and contribution guides',
            includePatterns: [
              'developer/references/references.md',
              'developer/references/iota-api/**',
              'developer/references/execution-architecture/**',
              'developer/references/research-papers.md',
              'developer/references/iota-glossary.md',
              'developer/references/contribute/**',
            ],
            fullContent: true,
          },
        ],
      }
    ],
    path.resolve(__dirname, './src/plugins/llms-txt/index.ts'),
    [
      "@graphql-markdown/docusaurus",
      /** @type {import('@graphql-markdown/types').ConfigOptions} */
      {
        id:'mainnet',
        schema: "https://raw.githubusercontent.com/iotaledger/iota/refs/heads/mainnet/crates/iota-graphql-rpc/schema.graphql",
        rootPath: "../content", // docs will be generated under rootPath/baseURL
        baseURL: "developer/references/iota-api/iota-graphql/reference/",
        loaders: {
          UrlLoader: {
            module: "@graphql-tools/url-loader",
          }
        },
      },
    ],
    [
      "@graphql-markdown/docusaurus",
      /** @type {import('@graphql-markdown/types').ConfigOptions} */
      {
        id:'testnet',
        schema: "https://raw.githubusercontent.com/iotaledger/iota/refs/heads/testnet/crates/iota-graphql-rpc/schema.graphql",
        rootPath: "../content", // docs will be generated under rootPath/baseURL
        baseURL: "developer/references/iota-api/iota-graphql/reference/Testnet/",
        loaders: {
          UrlLoader: {
            module: "@graphql-tools/url-loader",
          }
        },
      },
    ],
    [
      "@graphql-markdown/docusaurus",
      /** @type {import('@graphql-markdown/types').ConfigOptions} */
      {
        id:'devnet',
        schema: "https://raw.githubusercontent.com/iotaledger/iota/refs/heads/devnet/crates/iota-graphql-rpc/schema.graphql",
        rootPath: "../content", // docs will be generated under rootPath/baseURL
        baseURL: "developer/references/iota-api/iota-graphql/reference/Devnet/",
        loaders: {
          UrlLoader: {
            module: "@graphql-tools/url-loader",
          }
        },
      },
    ],
    async function myPlugin(context, options) {
      return {
        name: "docusaurus-tailwindcss",
        configurePostCss(postcssOptions) {
          // Appends TailwindCSS and AutoPrefixer.
          postcssOptions.plugins.push(require("tailwindcss"));
          postcssOptions.plugins.push(require("autoprefixer"));
          return postcssOptions;
        },
      };
    },
    path.resolve(__dirname, `./src/plugins/descriptions`),
    [
      'docusaurus-plugin-typedoc',
      {
        id: 'ts-sdk',
        tsconfig: '../../sdk/typescript/tsconfig.json',
        entryPoints: [
          '../../sdk/typescript/src/bcs',
          '../../sdk/typescript/src/client',
          '../../sdk/typescript/src/cryptography',
          '../../sdk/typescript/src/faucet',
          '../../sdk/typescript/src/graphql',
          '../../sdk/typescript/src/keypairs/ed25519',
          '../../sdk/typescript/src/keypairs/secp256k1',
          '../../sdk/typescript/src/keypairs/secp256r1',
          '../../sdk/typescript/src/multisig',
          '../../sdk/typescript/src/transactions',
          '../../sdk/typescript/src/utils',
          '../../sdk/typescript/src/verify',
        ],
        out: '../content/developer/ts-sdk/typescript/api',
        ...typedocBaseConfig,
      },
    ],
    [
      'docusaurus-plugin-typedoc',
      {
        id: 'dapp-kit',
        tsconfig: '../../sdk/dapp-kit/tsconfig.json',
        entryPoints: ['../../sdk/dapp-kit/src'],
        out: '../content/developer/ts-sdk/dapp-kit/api',
        ...typedocBaseConfig,
      },
    ],
    [
      'docusaurus-plugin-typedoc',
      {
        id: 'kiosk',
        tsconfig: '../../sdk/kiosk/tsconfig.json',
        entryPoints: ['../../sdk/kiosk/src'],
        out: '../content/developer/ts-sdk/kiosk/api',
        ...typedocBaseConfig,
      },
    ],
    [
      'docusaurus-plugin-typedoc',
      {
        id: 'bcs',
        tsconfig: '../../sdk/bcs/tsconfig.json',
        entryPoints: ['../../sdk/bcs/src/index.ts'],
        out: '../content/developer/ts-sdk/bcs/api',
        ...typedocBaseConfig,
      },
    ],
    [
      'docusaurus-plugin-typedoc',
      {
        id: 'signers',
        tsconfig: '../../sdk/signers/tsconfig.json',
        entryPoints: ['../../sdk/signers/src/ledger/index.ts', '../../sdk/signers/src/webcrypto/index.ts'],
        out: '../content/developer/ts-sdk/signers/api',
        ...typedocBaseConfig,
      },
    ],
    [
      'docusaurus-plugin-typedoc',
      {
        id: 'isc-sdk',
        tsconfig: '../../sdk/isc-sdk/tsconfig.json',
        entryPoints: ['../../sdk/isc-sdk/src/index.ts'],
        out: '../content/developer/ts-sdk/isc-sdk/api',
        ...typedocBaseConfig,
      },
    ],
    [
      'docusaurus-plugin-typedoc',
      {
        id: 'graphql-transport',
        tsconfig: '../../sdk/graphql-transport/tsconfig.json',
        entryPoints: ['../../sdk/graphql-transport/src'],
        out: '../content/developer/ts-sdk/graphql-transport/api',
        ...typedocBaseConfig,
      },
    ],
    [
      'docusaurus-plugin-typedoc',
      {
        id: 'wallet-standard',
        tsconfig: '../../sdk/wallet-standard/tsconfig.json',
        entryPoints: ['../../sdk/wallet-standard/src'],
        out: '../content/developer/ts-sdk/wallet-standard/api',
        ...typedocBaseConfig,
      },
    ],
    [
      'docusaurus-plugin-typedoc',
      {
        id: 'ledgerjs-hw-app-iota',
        tsconfig: '../../sdk/ledgerjs-hw-app-iota/tsconfig.json',
        entryPoints: ['../../sdk/ledgerjs-hw-app-iota/src/Iota.ts'],
        out: '../content/developer/ts-sdk/ledgerjs-hw-app-iota/api',
        ...typedocBaseConfig,
      },
    ],
    [
      '@docusaurus/plugin-client-redirects',
      {
        createRedirects(existingPath) {
          const redirects = [
            {
              from: '/references/ts-sdk',
              to: '/developer/ts-sdk/typescript',
            },
            {
              from: '/references/iota-identity',
              to: '/developer/iota-identity/references',
            },
            {
              from: '/references',
              to: '/developer/references',
            },
            {
              from: '/iota-evm',
              to: '/developer/iota-evm',
            },
            {
              from: '/iota-identity',
              to: '/developer/iota-identity',
            },
            {
              from: '/ts-sdk',
              to: '/developer/ts-sdk/typescript',
            },
            {
              from: '/developer/ts-sdk',
              to: '/developer/ts-sdk/typescript',
            },
            {
              from: '/about-iota/wallets',
              to: '/users/wallets',
            },
            {
              from: '/about-iota/iota-wallet',
              to: '/users/iota-wallet',
            },
            {
              from: '/about-iota/wallet-dashboard',
              to: '/users/iota-wallet-dashboard',
            },
            {
              from: '/about-iota/iota-wallet/how-to/integrate-ledger',
              to: '/users/iota-wallet/how-to/import/ledger'
            }
          ];
          let paths = [];
          for (const redirect of redirects) {
            if (existingPath.startsWith(redirect.to)) {
              paths.push(existingPath.replace(redirect.to, redirect.from));
            }
          }
          return paths.length > 0 ? paths : undefined;
        },
      },
    ],
    'plugin-image-zoom',
    [
      'docusaurus-plugin-openapi-docs',
      {
        id: 'openapi',
        docsPluginId: 'classic',
        config: {
          coreApiV2: {
            specPath:
              'https://raw.githubusercontent.com/iotaledger/wasp/refs/heads/develop/clients/apiclient/api/openapi.yaml',
            outputDir: 
              '../content/developer/iota-evm/references/openapi',
            sidebarOptions: {
              groupPathsBy: 'tag',
            }
          }
        }
      }
    ],
    [
      '@docusaurus/plugin-google-gtag',
      {
        trackingID: 'G-SEE2W8WK21',
        anonymizeIP: true,
      },
    ],
  ],
  presets: [
    [
      "classic",
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({
        docs: {
          path: "../content",
          routeBasePath: "/",
          sidebarPath: require.resolve("./sidebars.js"),
          docItemComponent: "@theme/ApiItem", // Derived from docusaurus-theme-openapi
          docRootComponent: "@theme/DocRoot", // add @theme/DocRoot
          async sidebarItemsGenerator({
            isCategoryIndex: defaultCategoryIndexMatcher, // The default matcher implementation, given below
            defaultSidebarItemsGenerator,
            ...args
          }) {
            return defaultSidebarItemsGenerator({
              ...args,
              isCategoryIndex(doc) {
                if(doc.fileName === 'index' && doc.directories.includes('ts-sdk'))
                  return true;
                // No doc will be automatically picked as category index
                return false;
              },
            });
          },
          // the double docs below is a fix for having the path set to ../content
          editUrl: "https://github.com/iotaledger/iota/tree/develop/docs/docs",
          onInlineTags: "throw",
          
          /*disableVersioning: true,
          lastVersion: "current",
          versions: {
            current: {
              label: "Latest",
              path: "/",
            },
          },
          onlyIncludeVersions: [
            "current",
            "1.0.0",
          ],*/
          remarkPlugins: [
            [math,{singleDollarTextMath:false}],
            [
              require("@docusaurus/remark-plugin-npm2yarn"),
              { sync: true, converters: ["yarn", "pnpm"] },
            ],
            [codeImport, { rootDir: path.resolve(__dirname, `../../`) }],
          ],
          rehypePlugins: [
            katex,
            [require('rehype-jargon'), { jargon: jargonConfig}]
          ],
        },
        theme: {
          customCss: [
            require.resolve("./src/css/fonts.css"),
            require.resolve("./src/css/custom.css"),
          ],
        },
      }),
    ],
  ],
  stylesheets: [
    {
      href: "https://fonts.googleapis.com/css2?family=Inter:wght@400;500;700&display=swap",
      type: "text/css",
    },
    {
      href: "https://cdn.jsdelivr.net/npm/katex@0.13.24/dist/katex.min.css",
      type: "text/css",
      integrity:
        "sha384-odtC+0UGzzFL/6PNoE8rX/SPcQDXBJ+uRepguP4QkPCm2LBxH3FA3y+fKSiJ+AmM",
      crossorigin: "anonymous",
    },
    {
      href: "https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.5.1/css/all.min.css",
      type: "text/css",
    },
  ],
  themes: [
    '@docusaurus/theme-mermaid',
    '@saucelabs/theme-github-codeblock', 
    '@docusaurus/theme-live-codeblock',
    'docusaurus-theme-openapi-docs',
  ],
  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      algolia: {
        apiKey: '24b141ea7e65db2181463e44dbe564a5',
        appId: '9PMBZGRP3B',
        indexName: 'iota',
      },
      image: "img/iota-doc-og.png",
      docs: {
        sidebar: {
          autoCollapseCategories: false,
        },
      },
      colorMode: {
        defaultMode: "dark",
      },
      announcementBar: {
        id: "iota-notarization",
        content:
          'Discover <a target="_blank" rel="noopener noreferrer" href="/developer/iota-notarization">IOTA Notarization Alpha</a> a toolkit for creating and managing tamper-proof records.',
        isCloseable: true,
        backgroundColor: "var(--ifm-color-primary-head-darkest)",
        textColor: "var(--iota-white)",
      },
      navbar: {
        title: "",
        logo: {
          alt: "IOTA Docs Logo",
          src: "/logo/iota-logo.svg",
        },
        items: [
          {
            label: "About IOTA",
            to: "about-iota",
            className: 'navbar-icon-about',
          },
          {
            label: "Developers",
            to: "developer",
            className: 'navbar-icon-developer',
          },
          {
            label: "Operators",
            to: "operator",
            className: 'navbar-icon-operator',
          },
          {
            label: "Users",
            to: "users",
            className: 'navbar-icon-users',
          },
          {
            label: "Workshops",
            to: "developer/workshops",
            className: 'navbar-icon-workshops',
            position: 'right',
          },
          {
            type: 'custom-WalletConnectButton',
            position: 'right',
          },
        ],
      },
      footer: {
        style: "dark",
        logo: {
          alt: "IOTA Wiki Logo",
          src: "/logo/iota-logo.svg",
        },
        copyright: `Copyright © ${new Date().getFullYear()} <a href='https://www.iota.org/'>IOTA Stiftung</a>, licensed under <a href="https://github.com/iotaledger/iota/blob/develop/docs/site/LICENSE">CC BY 4.0</a>. 
                    The documentation on this website is adapted from the <a href='https://docs.sui.io/'>SUI Documentation</a>, © 2024 by <a href='https://sui.io/'>SUI Foundation</a>, licensed under <a href="https://github.com/MystenLabs/sui/blob/main/docs/site/LICENSE">CC BY 4.0</a>.`,
      },
      socials: [
        'https://www.youtube.com/c/iotafoundation',
        'https://www.github.com/iotaledger/',
        'https://discord.gg/iota-builders',
        'https://discord.iota.org/',
        'https://www.twitter.com/iota/',
        'https://www.reddit.com/r/iota/',
        'https://www.linkedin.com/company/iotafoundation/',
        'https://www.instagram.com/iotafoundation/',
      ],
      prism: {
        theme: themes.vsLight,
        darkTheme: themes.vsDark,
        additionalLanguages: ["rust", "typescript", "solidity", "move"],
      },
      imageZoom: {
        selector: '.markdown img',
        // Optional medium-zoom options
        // see: https://www.npmjs.com/package/medium-zoom#options
        options: {
          background: "var(--iota-imagezoom-options)",
        },
      }
    }),
};

export default config;
