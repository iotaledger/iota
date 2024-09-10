import { defineConfig } from 'vite';
import { resolve } from 'path';
import react from '@vitejs/plugin-react-swc';
import tsconfigPaths from 'vite-tsconfig-paths';
import dts from 'vite-plugin-dts';

const isPackageProduction = process.env.BUILD_ENV === 'package';
const packageConfig = {
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
};

const plugins = isPackageProduction
    ? [
          tsconfigPaths({
              root: __dirname,
          }),
          react(),
          dts({ rollupTypes: true }),
      ]
    : [
          tsconfigPaths({
              root: __dirname,
          }),
          react(),
      ];

const buildPackageConfig = {
    build: isPackageProduction ? packageConfig : {},
    plugins,
    resolve: {
        alias: [
            {
                find: 'fs',
                replacement: 'memfs',
            },
        ],
    },
};

export default defineConfig(buildPackageConfig);
