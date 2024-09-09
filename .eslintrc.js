// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module.exports = {
    plugins: ['@tanstack/query', 'unused-imports', 'prettier', 'header', 'require-extensions'],
    extends: [
        'eslint:recommended',
        'plugin:@tanstack/eslint-plugin-query/recommended',
        'prettier',
        'plugin:prettier/recommended',
        'plugin:import/typescript',
        'plugin:@typescript-eslint/recommended',
    ],
    settings: {
        react: {
            version: '18',
        },
        'import/resolver': {
            typescript: true,
        },
    },
    env: {
        es2020: true,
    },
    root: true,
    ignorePatterns: [
        'coverage',
        'apps/icons/src',
        'next-env.d.ts',
        'doc/book',
        'external-crates',
        'storybook-static',
        '**/*.config.js',
        '**/*.config.ts',
        '**/preprocess.mjs',
        '**/storybook-static',
        '**/node_modules',
        'sdk/build-scripts/src/build-package.ts',
        'sdk/build-scripts/src/build-dapp-kit.ts',
        'sdk/create-dapp/bin/index.js',
        '**/build',
        '**/dist/',
        '**/.next/',
        '**/.swc/',
        '**/out/',
        '**/*.md',
        '**/*.mdx',
        '**/*.yml',
        '**/*.yaml',
    ],
    rules: {
        'no-case-declarations': 'off',
        'no-implicit-coercion': [2, { number: true, string: true, boolean: false }],
        '@typescript-eslint/no-redeclare': 'off',
        '@typescript-eslint/ban-types': [
            'error',
            {
                types: {
                    Buffer: 'Buffer usage increases bundle size and is not consistently implemented on web.',
                },
                extendDefaults: true,
            },
        ],
        'no-restricted-globals': [
            'error',
            {
                name: 'Buffer',
                message:
                    'Buffer usage increases bundle size and is not consistently implemented on web.',
            },
        ],
        'header/header': [
            2,
            'line',
            [
                {
                    pattern: ' Copyright \\(c\\) (2024 IOTA Stiftung|Mysten Labs, Inc.)?',
                },
                {
                    pattern:
                        ' ((SPDX-License-Identifier: Apache-2.0)|(Modifications Copyright \\(c\\) 2024 IOTA Stiftung))',
                },
            ],
        ],
        '@typescript-eslint/no-unused-vars': [
            'error',
            {
                argsIgnorePattern: '^_',
                varsIgnorePattern: '^_',
                vars: 'all',
                args: 'none',
                ignoreRestSiblings: true,
            },
        ],
    },
    overrides: [
        {
            files: ['sdk/**/*'],
            rules: {
                'require-extensions/require-extensions': 'error',
                'require-extensions/require-index': 'error',
                '@typescript-eslint/consistent-type-imports': ['error'],
                'import/consistent-type-specifier-style': ['error', 'prefer-top-level'],
                'import/no-cycle': ['error'],
                '@typescript-eslint/no-explicit-any': 'off',
            },
        },
        {
            files: ['sdk/graphql-transport/**/*'],
            rules: {
                '@typescript-eslint/no-non-null-asserted-optional-chain': 'off',
            },
        },
        {
            files: ['apps/explorer/**/*'],
            rules: {
                'import/no-duplicates': ['error'],
                'import/no-anonymous-default-export': 'off',
                '@typescript-eslint/consistent-type-imports': [
                    'error',
                    {
                        prefer: 'type-imports',
                        disallowTypeAnnotations: true,
                        fixStyle: 'inline-type-imports',
                    },
                ],
                '@typescript-eslint/unified-signatures': 'error',
                '@typescript-eslint/parameter-properties': 'error',
                'react/jsx-key': ['error', {}],

                'react/boolean-prop-naming': 'off',
                'react/jsx-boolean-value': ['error', 'never'],

                // Always use function declarations for components
                'react/function-component-definition': [
                    'error',
                    {
                        namedComponents: 'function-declaration',
                        unnamedComponents: 'arrow-function',
                    },
                ],
                'react/prefer-stateless-function': 'error',
                'react/jsx-pascal-case': ['error', { allowAllCaps: true, allowNamespace: true }],

                // Always self-close when applicable
                'react/self-closing-comp': [
                    'error',
                    {
                        component: true,
                        html: true,
                    },
                ],
                'react/void-dom-elements-no-children': 'error',

                // Use alternatives instead of danger
                'react/no-danger': 'error',
                'react/no-danger-with-children': 'error',

                // Accessibility requirements
                'react/button-has-type': 'error',
                'react/no-invalid-html-attribute': 'error',

                // Security requirements
                'react/jsx-no-script-url': 'error',
                'react/jsx-no-target-blank': 'error',

                // Enforce consistent JSX spacing and syntax
                'react/jsx-no-comment-textnodes': 'error',
                'react/jsx-no-duplicate-props': 'error',
                'react/jsx-no-undef': 'error',
                'react/jsx-space-before-closing': 'off',

                // Avoid interpolation as much as possible
                'react/jsx-curly-brace-presence': ['error', { props: 'never', children: 'never' }],

                // Always use shorthand fragments when applicable
                'react/jsx-fragments': ['error', 'syntax'],
                'react/jsx-no-useless-fragment': ['error', { allowExpressions: true }],
                'react/jsx-handler-names': [
                    'error',
                    {
                        eventHandlerPropPrefix: 'on',
                    },
                ],

                // Avoid bad or problematic patterns
                'react/jsx-uses-vars': 'error',
                'react/no-access-state-in-setstate': 'error',
                'react/no-arrow-function-lifecycle': 'error',
                'react/no-children-prop': 'error',
                'react/no-did-mount-set-state': 'error',
                'react/no-did-update-set-state': 'error',
                'react/no-direct-mutation-state': 'error',
                'react/no-namespace': 'error',
                'react/no-redundant-should-component-update': 'error',
                'react/no-render-return-value': 'error',
                'react/no-string-refs': 'error',
                'react/no-this-in-sfc': 'error',
                'react/no-typos': 'error',
                'react/no-unescaped-entities': 'error',
                'react/no-unknown-property': 'error',
                'react/no-unused-class-component-methods': 'error',
                'react/no-will-update-set-state': 'error',
                'react/require-optimization': 'off',
                'react/style-prop-object': 'error',
                'react/no-unstable-nested-components': 'error',

                // We may eventually want to turn this on but it requires migration:
                'react/no-array-index-key': 'off',

                // Require usage of the custom Link component:
                'no-restricted-imports': [
                    'error',
                    {
                        paths: [
                            {
                                name: 'react-router-dom',
                                importNames: ['Link', 'useNavigate', 'useSearchParams'],
                                message:
                                    'Please use `LinkWithQuery`, `useSearchParamsMerged`, and `useNavigateWithQuery` from "~/components/ui/LinkWithQuery" instead.',
                            },
                        ],
                    },
                ],
                'arrow-body-style': ['error', 'as-needed'],
            },
        },
        {
            files: ['apps/wallet/**/*'],
            rules: {
                'react/display-name': 'off',
                'import/no-duplicates': ['error'],
                '@typescript-eslint/consistent-type-imports': [
                    'error',
                    {
                        prefer: 'type-imports',
                        disallowTypeAnnotations: true,
                        fixStyle: 'inline-type-imports',
                    },
                ],
                '@typescript-eslint/unified-signatures': 'error',
                '@typescript-eslint/parameter-properties': 'error',
                'no-console': ['warn'],
                '@typescript-eslint/no-non-null-assertion': 'off',
            },
        },
        {
            files: ['dapps/kiosk/**/*'],
            rules: {
                'no-unused-vars': 'off', // or "@typescript-eslint/no-unused-vars": "off",
                'unused-imports/no-unused-imports': 'error',
                'unused-imports/no-unused-vars': [
                    'warn',
                    {
                        vars: 'all',
                        varsIgnorePattern: '^_',
                        args: 'after-used',
                        argsIgnorePattern: '^_',
                    },
                ],
            },
        },
        {
            files: ['sdk/ledgerjs-hw-app-iota/**/*', 'apps/wallet/**/*'],
            rules: {
                // ledgerjs-hw-app-iota and wallet use Buffer
                'no-restricted-globals': ['off'],
                '@typescript-eslint/ban-types': ['off'],
            },
        },
        {
            files: ['*.test.*', '*.spec.*'],
            rules: {
                // Tests can violate extension rules:
                'require-extensions/require-extensions': 'off',
                'require-extensions/require-index': 'off',
                '@typescript-eslint/consistent-type-imports': ['off'],
                'import/consistent-type-specifier-style': ['off'],
                // Reset to defaults to allow `Buffer` usage in tests (given they run in Node and do not impact bundle):
                'no-restricted-globals': ['off'],
                '@typescript-eslint/ban-types': ['error'],
                '@typescript-eslint/no-explicit-any': 'off',
                '@typescript-eslint/no-non-null-asserted-optional-chain': 'off',
            },
        },
        {
            files: ['*.stories.*'],
            rules: {
                // Story files have render functions that this rule incorrectly warns on:
                'react-hooks/rules-of-hooks': 'off',
            },
        },
        {
            files: ['sdk/create-dapp/templates/**/*'],
            rules: {
                'header/header': 'off',
                'require-extensions/require-extensions': 'off',
            },
        },
        {
            files: ['apps/apps-backend/**/*'],
            env: {
                node: true,
                jest: true,
            },
        },
        {
            files: ['apps/wallet-dashboard/**/*'],
            extends: 'next/core-web-vitals',
        },
    ],
};
