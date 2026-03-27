import { sentryVitePlugin } from '@sentry/vite-plugin';
import react from '@vitejs/plugin-react-swc';
import { execSync } from 'child_process';
import { defineConfig, loadEnv } from 'vite';

const COMMIT_REV = execSync('git rev-parse HEAD').toString().trim().toString();

process.env.VITE_VERCEL_ENV = process.env.VERCEL_ENV || 'development';

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => {
    const env = loadEnv(mode, process.cwd(), '');
    const sentryAuthToken = env.SENTRY_AUTH_TOKEN;
    const IS_PROD = env.VITE_BUILD_ENV === 'production';
    return {
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
                silent: !env.CI,
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
            'process.env.APPS_BACKEND': JSON.stringify(process.env.APPS_BACKEND ?? ''),
        },
    };
});
