import { defineConfig } from 'vite';
import { resolve } from 'node:path';
import dts from 'vite-plugin-dts';

// https://vitejs.dev/config/
export default defineConfig({
    build: {
        lib: {
            entry: resolve(__dirname, './src/index.ts'),
            name: '@iota/isc-sdk',
            fileName: 'index',
            formats: ['es', 'cjs'],
        },
    },
    plugins: [dts()],
});
