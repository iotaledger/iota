import eslint from '@eslint/js';
import tseslint from 'typescript-eslint';

export default tseslint.config(
    {
        files: ['**/*.{js,mjs,cjs,ts}'],
        settings: {
            version: '18',
        },
    },
    eslint.configs.recommended,
    tseslint.configs.recommended,
);
