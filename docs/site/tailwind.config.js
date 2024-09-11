// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module.exports = {
  corePlugins: {
    preflight: false, // disable Tailwind's reset
  },
  content: ["./src/**/*.{js,jsx,ts,tsx}", "./docs/**/*.mdx"], // my markdown stuff is in ../docs, not /src
  darkMode: ["class", '[data-theme="dark"]'], // hooks into docusaurus' dark mode settings
  theme: {
    extend: {
      fontFamily: {
        twkeverett: ["Twkeverett"],
      },
      colors: {
        "iota-black": "var(--iota-black)",
        "iota-blue": "var(--iota-blue)",
        "iota-blue-bright": "var(--iota-blue-bright)",
        "iota-blue-light": "var(--iota-blue-light)",
        "iota-blue-lighter": "var(--iota-blue-lighter)",
        "iota-blue-dark": "rgb(var(--iota-blue-dark)/<alpha-value>)",
        "iota-blue-darker": "var(--iota-blue-darker)",
        "iota-hero": "var(--iota-hero)",
        "iota-hero-dark": "var(--iota-hero-dark)",
        "iota-steel": "var(--iota-steel)",
        "iota-steel-dark": "var(--iota-steel-dark)",
        "iota-steel-darker": "var(--iota-steel-darker)",
        "iota-header-nav": "var(--iota-header-nav)",
        "iota-success": "var(--iota-success)",
        "iota-success-dark": "var(--iota-success-dark)",
        "iota-success-light": "var(--iota-success-light)",
        "iota-issue": "var(--iota-issue)",
        "iota-issue-dark": "var(--iota-issue-dark)",
        "iota-issue-light": "var(--iota-issue-light)",
        "iota-warning": "var(--iota-warning)",
        "iota-warning-dark": "var(--iota-warning-dark)",
        "iota-warning-light": "var(--iota-warning-light)",
        "iota-code": "var(--iota-code)",
        "iota-gray": {
          35: "var(--iota-gray-35)",
          40: "var(--iota-gray-40)",
          45: "var(--iota-gray-45)",
          50: "var(--iota-gray-50)",
          55: "var(--iota-gray-55)",
          60: "var(--iota-gray-60)",
          65: "var(--iota-gray-65)",
          70: "var(--iota-gray-70)",
          75: "var(--iota-gray-75)",
          80: "var(--iota-gray-80)",
          85: "var(--iota-gray-85)",
          90: "var(--iota-gray-90)",
          95: "var(--iota-gray-95)",
          100: "var(--iota-gray-100)",
        },
        "iota-grey": {
          35: "var(--iota-gray-35)",
          40: "var(--iota-gray-40)",
          45: "var(--iota-gray-45)",
          50: "var(--iota-gray-50)",
          55: "var(--iota-gray-55)",
          60: "var(--iota-gray-60)",
          65: "var(--iota-gray-65)",
          70: "var(--iota-gray-70)",
          75: "var(--iota-gray-75)",
          80: "var(--iota-gray-80)",
          85: "var(--iota-gray-85)",
          90: "var(--iota-gray-90)",
          95: "var(--iota-gray-95)",
          100: "var(--iota-gray-100)",
        },
        "iota-link-color-dark": "var(--iota-link-color-dark)",
        "iota-link-color-light": "var(--iota-link-color-light)",
        "iota-ghost-white": "var(--iota-ghost-white)",
        "iota-ghost-dark": "var(--iota-ghost-dark)",
        "ifm-background-color-dark": "var(--ifm-background-color-dark)",
        "iota-white": "rgb(var(--iota-white)/<alpha-value>)",
        "iota-card-dark": "rgb(var(--iota-card-dark)/<alpha-value>)",
        "iota-card-darker": "rgb(var(--iota-card-darker)/<alpha-value>)",
      },
      borderRadius: {
        iota: "40px",
      },
      boxShadow: {
        iota: "0px 0px 4px rgba(0, 0, 0, 0.02)",
        "iota-button": "0px 1px 2px rgba(16, 24, 40, 0.05)",
        "iota-notification": "0px 0px 20px rgba(29, 55, 87, 0.11)",
      },
      gradientColorStopPositions: {
        36: "36%",
      },
    },
  },
  plugins: [],
};
