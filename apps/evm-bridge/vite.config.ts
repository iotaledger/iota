import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react-swc';
import { execSync } from 'child_process';
import { sentryVitePlugin } from '@sentry/vite-plugin';

const COMMIT_REV = execSync('git rev-parse HEAD').toString().trim().toString();

process.env.VITE_VERCEL_ENV = process.env.VERCEL_ENV || 'development';
process.env.VITE_BUILD_ENV = process.env.BUILD_ENV || 'development';

const sentryAuthToken = process.env.SENTRY_AUTH_TOKEN;
const IS_PROD = process.env.BUILD_ENV === 'production';

// https://vitejs.dev/config/
export default defineConfig({
    plugins: [
        react(),
        sentryVitePlugin({
            org: 'iota-foundation-eu',
            project: 'iota-evm-bridge',
            authToken: sentryAuthToken,
            sourcemaps: {
                assets: './dist/**',
            },
            disable: !IS_PROD || !sentryAuthToken,
            silent: !process.env.CI,
            release: {
                name: COMMIT_REV,
            },
        }),
    ],
    build: {
        sourcemap: true,
    },
    define: {
        COMMIT_REV: JSON.stringify(COMMIT_REV),
    },
});
