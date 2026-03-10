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
        node: true,
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
};
