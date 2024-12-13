// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { themes } from "prism-react-renderer";
import path from "path";
import math from "remark-math";
import katex from "rehype-katex";
import codeImport from "remark-code-import";

require("dotenv").config();

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: "IOTA Documentation",
  tagline:
    "IOTA is a next-generation smart contract platform with high throughput, low latency, and an asset-oriented programming model powered by Move",
  favicon: "/icons/favicon.ico",

  // Set the production url of your site here
  url: "https://docs.iota.org",
  // Set the /<baseUrl>/ pathname under which your site is served
  // For GitHub pages deployment, it is often '/<projectName>/'
  baseUrl: "/",
  customFields: {
    amplitudeKey: process.env.AMPLITUDE_KEY,
  },

  onBrokenLinks: "throw",
  onBrokenMarkdownLinks: "throw",
  onBrokenAnchors: "throw",

  // Even if you don't use internationalization, you can use this field to set
  // useful metadata like html lang. For example, if your site is Chinese, you
  // may want to replace "en" with "zh-Hans".
  /*  i18n: {
    defaultLocale: "en",
    locales: [
      "en",
      "el",
      "fr",
      "ko",
      "tr",
      "vi",
      "zh-CN",
      "zh-TW",
    ],
  },*/
  markdown: {
    format: "detect",
    mermaid: true,
  },
  plugins: [
    // ....
    [
      "@graphql-markdown/docusaurus",
      {
        schema:
          "../../crates/iota-graphql-rpc/schema.graphql",
        rootPath: "../content", // docs will be generated under rootPath/baseURL
        baseURL: "references/iota-api/iota-graphql/reference",
        loaders: {
          GraphQLFileLoader: "@graphql-tools/graphql-file-loader",
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
      // Options
      {
        tsconfig: '../../sdk/typescript/tsconfig.json',
        entryPoints: [
          "../../sdk/typescript/src/bcs",
          "../../sdk/typescript/src/client",
          "../../sdk/typescript/src/cryptography",
          "../../sdk/typescript/src/faucet",
          "../../sdk/typescript/src/graphql",
          "../../sdk/typescript/src/keypairs/ed25519",
          "../../sdk/typescript/src/keypairs/secp256k1",
          "../../sdk/typescript/src/keypairs/secp256k1",
          "../../sdk/typescript/src/multisig",
          "../../sdk/typescript/src/transactions",
          "../../sdk/typescript/src/utils",
          "../../sdk/typescript/src/verify"
        ],
        plugin: ["typedoc-plugin-markdown"],
        out: "../../docs/content/ts-sdk/api/",
        githubPages: false,
        readme: "none",
        hideGenerator: true,
        sort: ["source-order"],
        excludeInternal: true,
        excludePrivate: true,
        disableSources: true,
        hideBreadcrumbs: true,
        intentionallyNotExported: [],
      },
    ],
    [
      '@docusaurus/plugin-client-redirects',
      {
        createRedirects(existingPath) {
          const redirects = [
            {
              to: 'ts-sdk/typescript/install',
              from: '/references/ts-sdk/typescript/install',
          },
          {
              to: 'ts-sdk/typescript/hello-iota',
              from: '/references/ts-sdk/typescript/hello-iota',
          },
          {
              to: 'ts-sdk/typescript/faucet',
              from: '/references/ts-sdk/typescript/faucet',
          },
          {
              to: 'ts-sdk/typescript/iota-client',
              from: '/references/ts-sdk/typescript/iota-client',
          },
          {
              to: 'ts-sdk/typescript/graphql',
              from: '/references/ts-sdk/typescript/graphql',
          },
          {
              to: 'ts-sdk/typescript/transaction-building/basics',
              from: '/references/ts-sdk/typescript/transaction-building/basics',
          },
          {
              to: 'ts-sdk/typescript/transaction-building/gas',
              from: '/references/ts-sdk/typescript/transaction-building/gas',
          },
          {
              to: 'ts-sdk/typescript/transaction-building/sponsored-transactions',
              from: '/references/ts-sdk/typescript/transaction-building/sponsored-transactions',
          },
          {
              to: 'ts-sdk/typescript/transaction-building/offline',
              from: '/references/ts-sdk/typescript/transaction-building/offline',
          },
          {
              to: 'ts-sdk/typescript/cryptography/keypairs',
              from: '/references/ts-sdk/typescript/cryptography/keypairs',
          },
          {
              to: 'ts-sdk/typescript/cryptography/multisig',
              from: '/references/ts-sdk/typescript/cryptography/multisig',
          },
          {
              to: 'ts-sdk/typescript/utils',
              from: '/references/ts-sdk/typescript/utils',
          },
          {
              to: 'ts-sdk/typescript/bcs',
              from: '/references/ts-sdk/typescript/bcs',
          },
          {
              to: 'ts-sdk/typescript/executors',
              from: '/references/ts-sdk/typescript/executors',
          },
          {
              to: 'ts-sdk/typescript/plugins',
              from: '/references/ts-sdk/typescript/plugins',
          },
          {
              to: 'ts-sdk/typescript/owned-object-pool/overview',
              from: '/references/ts-sdk/typescript/owned-object-pool/overview',
          },
          {
              to: 'ts-sdk/typescript/owned-object-pool/local-development',
              from: '/references/ts-sdk/typescript/owned-object-pool/local-development',
          },
          {
              to: 'ts-sdk/typescript/owned-object-pool/custom-split-strategy',
              from: '/references/ts-sdk/typescript/owned-object-pool/custom-split-strategy',
          },
          {
              to: 'ts-sdk/typescript/owned-object-pool/examples',
              from: '/references/ts-sdk/typescript/owned-object-pool/examples',
          },
          {
              to: 'ts-sdk/dapp-kit/create-dapp',
              from: '/references/ts-sdk/dapp-kit/create-dapp',
          },
          {
              to: 'ts-sdk/dapp-kit/iota-client-provider',
              from: '/references/ts-sdk/dapp-kit/iota-client-provider',
          },
          {
              to: 'ts-sdk/dapp-kit/rpc-hooks',
              from: '/references/ts-sdk/dapp-kit/rpc-hooks',
          },
          {
              to: 'ts-sdk/dapp-kit/wallet-provider',
              from: '/references/ts-sdk/dapp-kit/wallet-provider',
          },
          {
              to: 'ts-sdk/dapp-kit/wallet-components/ConnectButton',
              from: '/references/ts-sdk/dapp-kit/wallet-components/ConnectButton',
          },
          {
              to: 'ts-sdk/dapp-kit/wallet-components/ConnectModal',
              from: '/references/ts-sdk/dapp-kit/wallet-components/ConnectModal',
          },
          {
              to: 'ts-sdk/dapp-kit/wallet-hooks/useWallets',
              from: '/references/ts-sdk/dapp-kit/wallet-hooks/useWallets',
          },
          {
              to: 'ts-sdk/dapp-kit/wallet-hooks/useAccounts',
              from: '/references/ts-sdk/dapp-kit/wallet-hooks/useAccounts',
          },
          {
              to: 'ts-sdk/dapp-kit/wallet-hooks/useCurrentWallet',
              from: '/references/ts-sdk/dapp-kit/wallet-hooks/useCurrentWallet',
          },
          {
              to: 'ts-sdk/dapp-kit/wallet-hooks/useCurrentAccount',
              from: '/references/ts-sdk/dapp-kit/wallet-hooks/useCurrentAccount',
          },
          {
              to: 'ts-sdk/dapp-kit/wallet-hooks/useAutoConnectWallet',
              from: '/references/ts-sdk/dapp-kit/wallet-hooks/useAutoConnectWallet',
          },
          {
              to: 'ts-sdk/dapp-kit/wallet-hooks/useConnectWallet',
              from: '/references/ts-sdk/dapp-kit/wallet-hooks/useConnectWallet',
          },
          {
              to: 'ts-sdk/dapp-kit/wallet-hooks/useDisconnectWallet',
              from: '/references/ts-sdk/dapp-kit/wallet-hooks/useDisconnectWallet',
          },
          {
              to: 'ts-sdk/dapp-kit/wallet-hooks/useSwitchAccount',
              from: '/references/ts-sdk/dapp-kit/wallet-hooks/useSwitchAccount',
          },
          {
              to: 'ts-sdk/dapp-kit/wallet-hooks/useReportTransactionEffects',
              from: '/references/ts-sdk/dapp-kit/wallet-hooks/useReportTransactionEffects',
          },
          {
              to: 'ts-sdk/dapp-kit/wallet-hooks/useSignPersonalMessage',
              from: '/references/ts-sdk/dapp-kit/wallet-hooks/useSignPersonalMessage',
          },
          {
              to: 'ts-sdk/dapp-kit/wallet-hooks/useSignTransaction',
              from: '/references/ts-sdk/dapp-kit/wallet-hooks/useSignTransaction',
          },
          {
              to: 'ts-sdk/dapp-kit/wallet-hooks/useSignAndExecuteTransaction',
              from: '/references/ts-sdk/dapp-kit/wallet-hooks/useSignAndExecuteTransaction',
          },
          {
              to: 'ts-sdk/dapp-kit/themes',
              from: '/references/ts-sdk/dapp-kit/themes',
          },
          {
              to: 'ts-sdk/kiosk/kiosk-client/introduction',
              from: '/references/ts-sdk/kiosk/kiosk-client/introduction',
          },
          {
              to: 'ts-sdk/kiosk/kiosk-client/querying',
              from: '/references/ts-sdk/kiosk/kiosk-client/querying',
          },
          {
              to: 'ts-sdk/kiosk/kiosk-client/kiosk-transaction/kiosk-transaction',
              from: '/references/ts-sdk/kiosk/kiosk-client/kiosk-transaction/kiosk-transaction',
          },
          {
              to: 'ts-sdk/kiosk/kiosk-client/kiosk-transaction/managing',
              from: '/references/ts-sdk/kiosk/kiosk-client/kiosk-transaction/managing',
          },
          {
              to: 'ts-sdk/kiosk/kiosk-client/kiosk-transaction/purchasing',
              from: '/references/ts-sdk/kiosk/kiosk-client/kiosk-transaction/purchasing',
          },
          {
              to: 'ts-sdk/kiosk/kiosk-client/kiosk-transaction/examples',
              from: '/references/ts-sdk/kiosk/kiosk-client/kiosk-transaction/examples',
          },
          {
              to: 'ts-sdk/kiosk/kiosk-client/transfer-policy-transaction/introduction',
              from: '/references/ts-sdk/kiosk/kiosk-client/transfer-policy-transaction/introduction',
          },
          {
              to: 'ts-sdk/kiosk/kiosk-client/transfer-policy-transaction/using-the-manager',
              from: '/references/ts-sdk/kiosk/kiosk-client/transfer-policy-transaction/using-the-manager',
          },
          {
              to: 'ts-sdk/kiosk/advanced-examples',
              from: '/references/ts-sdk/kiosk/advanced-examples',
          },
          {
              to: 'ts-sdk/bcs',
              from: '/references/ts-sdk/bcs',
          },
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
    'plugin-image-zoom'
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
            math,
            [
              require("@docusaurus/remark-plugin-npm2yarn"),
              { sync: true, converters: ["yarn", "pnpm"] },
            ],
            [codeImport, { rootDir: path.resolve(__dirname, `../../`) }],
          ],
          rehypePlugins: [katex],
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
  themes: ["@docusaurus/theme-live-codeblock", "@docusaurus/theme-mermaid", 'docusaurus-theme-search-typesense',
    '@saucelabs/theme-github-codeblock'],
  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      typesense: {
        // Replace this with the name of your index/collection.
        // It should match the "index_name" entry in the scraper's "config.json" file.
        typesenseCollectionName: 'IOTADocs',
        typesenseServerConfig: {
          nodes: [
            {
              host: 'docs-search.iota.org',
              port: '',
              protocol: 'https',
            },
          ],
          apiKey: 'C!jA3iCujG*PjK!eUVWFBxnU',
        },
        // Optional: Typesense search parameters: https://typesense.org/docs/0.24.0/api/search.html#search-parameters
        typesenseSearchParameters: {},
        // Optional
        contextualSearch: true,
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
        id: "integrate_your_exchange",
        content:
          '<a target="_blank" rel="noopener noreferrer" href="/developer/exchange-integration/">Integrate your exchange</a>. If you supported Stardust, please make sure to also <a target="_blank" rel="noopener noreferrer" href="/developer/stardust/exchanges"> migrate from Stardust</a>.',
        isCloseable: false,
        backgroundColor: "#0101ff",
        textColor: "#FFFFFF",
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
          },
          {
            label: "Developers",
            to: "developer",
          },
          {
            label: "Node Operators",
            to: "operator",
          },
          {
            label: "References",
            to: "references",
          },
          {
            label: "TS SDK",
            to: "ts-sdk/typescript/",
          },
          {
            label: "IOTA Identity",
            to: "iota-identity",
          },
        ],
      },
      footer: {
        logo: {
          alt: "IOTA Wiki Logo",
          src: "/logo/iota-logo.svg",
        },
        copyright: `Copyright © ${new Date().getFullYear()} <a href='https://www.iota.org/'>IOTA Stiftung</a>, licensed under <a href="https://github.com/iotaledger/iota/blob/main/docs/site/LICENSE">CC BY 4.0</a>. 
                    The documentation on this website is adapted from the <a href='https://docs.sui.io/'>SUI Documentation</a>, © 2024 by <a href='https://sui.io/'>SUI Foundation</a>, licensed under <a href="https://github.com/MystenLabs/sui/blob/main/docs/site/LICENSE">CC BY 4.0</a>.`,
      },
      socials: [
        'https://www.youtube.com/c/iotafoundation',
        'https://www.github.com/iotaledger/',
        'https://discord.iota.org/',
        'https://www.twitter.com/iota/',
        'https://www.reddit.com/r/iota/',
        'https://www.linkedin.com/company/iotafoundation/',
        'https://www.instagram.com/iotafoundation/',
      ],
      prism: {
        theme: themes.vsLight,
        darkTheme: themes.vsDark,
        additionalLanguages: ["rust", "typescript", "solidity"],
      },
      imageZoom: {
        selector: '.markdown img',
        // Optional medium-zoom options
        // see: https://www.npmjs.com/package/medium-zoom#options
        options: {
          background: 'rgba(0, 0, 0, 0.6)',
        },
      }
    }),
};

export default config;
