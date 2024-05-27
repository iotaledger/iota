// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

module.exports = {
    printWidth: 100,
    semi: true,
    singleQuote: true,
    tabWidth: 4,
    trailingComma: 'all',
    useTabs: false,
    plugins: ['@ianvs/prettier-plugin-sort-imports'],
    importOrder: [
        '<BUILT_IN_MODULES>',
        '<THIRD_PARTY_MODULES>',
        '',
        '^@/(.*)$',
        '^~/(.*)$',
        '',
        '^[.]',
    ],
    importOrderParserPlugins: ['typescript', 'decorators-legacy'],
    overrides: [
        {
            files: ['apps/explorer/**/*', 'apps/wallet/**/*'],
            options: {
                plugins: ['prettier-plugin-tailwindcss'],
                tailwindConfig: './apps/explorer/tailwind.config.ts',
            },
        },
        {
            files: 'apps/wallet-dashboard/**/*',
            options: {
                plugins: ['prettier-plugin-tailwindcss'],
                tailwindConfig: './apps/wallet-dashboard/tailwind.config.ts',
            },
        },
        {
            files: 'sdk/**/*',
            options: {
                proseWrap: 'always',
            },
        },
        {
            files: 'apps/wallet/**/*',
            options: {
                tabWidth: 2,
                useTabs: true,
            },
        },
        {
            files: 'external-crates/move/documentation/book/**/*',
            options: {
                proseWrap: 'always',
            },
        },
    ],
};
