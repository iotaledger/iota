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
  favicon: "/img/favicon.ico",

  // Set the production url of your site here
  url: "https://docs.iota.io",
  // Set the /<baseUrl>/ pathname under which your site is served
  // For GitHub pages deployment, it is often '/<projectName>/'
  baseUrl: "/",
  customFields: {
    amplitudeKey: process.env.AMPLITUDE_KEY,
  },

  onBrokenLinks: "throw",
  onBrokenMarkdownLinks: "throw",
  onBrokenAnchors: "warn",

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
          "../../crates/iota-graphql-rpc/schema/current_progress_schema.graphql",
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
  themes: ["@docusaurus/theme-mermaid"],
  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      algolia: {
        // The application ID provided by Algolia
        appId: "ZF283DJAYX",

        // Public API key: it is safe to commit it
        apiKey: "7f24db6c4ec06d6905592deb228f4460",

        indexName: "iota",

        // Optional: see doc section below
        contextualSearch: false,

        // Optional: Specify domains where the navigation should occur through window.location instead on history.push. Useful when our Algolia config crawls multiple documentation sites and we want to navigate with window.location.href to them.
        // externalUrlRegex: "external\\.com|domain\\.com",

        // Optional: Replace parts of the item URLs from Algolia. Useful when using the same search index for multiple deployments using a different baseUrl. You can use regexp or string in the `from` param. For example: localhost:3000 vs myCompany.com/docs
        //replaceSearchResultPathname: {
        //from: "/docs/", // or as RegExp: /\/docs\//
        //to: "/",
        //},

        // Optional: Algolia search parameters
        //searchParameters: {},

        // Optional: path for search page that enabled by default (`false` to disable it)
        searchPagePath: "search",

        //... other Algolia params
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
          src: "img/iota-logo.svg",
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
        ],
      },
      colorMode: {
        defaultMode: "dark",
        disableSwitch: false,
      },
      footer: {
        logo: {
          alt: "IOTA Wiki Logo",
          src: "img/iota-logo.svg",
        },
        copyright: `Copyright © ${new Date().getFullYear()} <a href='https://www.iota.org/'>IOTA Stiftung</a>, licensed under <a href="https://github.com/iotaledger/iota/blob/main/docs/site/LICENSE">CC BY 4.0</a>. 
                    The documentation on this website is adapted from the <a href='https://docs.sui.io/'>SUI Documentation</a>, © 2024 by <a href='https://sui.io/'>SUI Foundation</a>, licensed under <a href="https://github.com/MystenLabs/sui/blob/main/docs/site/LICENSE">CC BY 4.0</a>.`,
      },
      prism: {
        theme: themes.github,
        darkTheme: themes.jettwaveDark,
        additionalLanguages: ["rust", "typescript", "toml", "solidity"],
      },
    }),
};

export default config;
