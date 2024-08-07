// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module.exports = {
    printWidth: 100,
    semi: true,
    singleQuote: true,
    tabWidth: 4,
    trailingComma: 'all',
    useTabs: false,
    overrides: [
        {
            files: 'apps/explorer/**/*',
            options: {
                plugins: ['prettier-plugin-tailwindcss'],
                tailwindConfig: './apps/explorer/tailwind.config.ts',
            },
        },
        {
            files: 'apps/wallet/**/*',
            options: {
                plugins: ['prettier-plugin-tailwindcss'],
                tailwindConfig: './apps/wallet/tailwind.config.ts',
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
            files: 'apps/ui-kit/**/*',
            options: {
                plugins: ['prettier-plugin-tailwindcss'],
                tailwindConfig: './apps/ui-kit/tailwind.config.ts',
            },
        },
        {
            files: 'sdk/**/*',
            options: {
                proseWrap: 'always',
            },
        },
        {
            files: 'external-crates/move/documentation/book/**/*',
            options: {
                proseWrap: 'always',
            },
        },
        {
            files: 'docs/**/*',
            options: {
                proseWrap: 'always',
            },
        },
    ],
};
