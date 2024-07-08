import { defineConfig } from 'vite';
import { resolve } from 'path';
import react from '@vitejs/plugin-react-swc';
import tsconfigPaths from 'vite-tsconfig-paths';
import dts from 'vite-plugin-dts';
import tailwindcss from 'tailwindcss';

export default defineConfig({
    build: {
        lib: {
            entry: resolve(__dirname, './src/lib/index.ts'),
            name: '@iota/apps-ui-kit',
            fileName: (format) => `index.${format}.js`,
        },
        rollupOptions: {
            external: ['react', 'react-dom', 'tailwindcss'],
            output: {
                globals: {
                    react: 'React',
                    'react-dom': 'ReactDOM',
                    tailwindcss: 'tailwindcss',
                },
            },
        },
        sourcemap: true,
        emptyOutDir: true,
    },
    plugins: [
        tsconfigPaths({
            root: __dirname,
        }),
        react(),
        dts({ rollupTypes: true }),
    ],
    css: {
        postcss: {
            plugins: [tailwindcss],
        },
    },
});
