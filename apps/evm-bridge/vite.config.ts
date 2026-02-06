import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react-swc';
import { execSync } from 'child_process';
import { sentryVitePlugin } from '@sentry/vite-plugin';
import { SENTRY_ORG_NAME, SENTRY_PROJECT_NAME } from './sentry.config';

const COMMIT_REV = execSync('git rev-parse HEAD').toString().trim().toString();
const VITE_SENTRY_BUILD_ENV = process.env.VITE_SENTRY_BUILD_ENV || process.env.VITE_BUILD_ENV;

// https://vitejs.dev/config/
export default defineConfig({
    plugins: [
        react(),
        sentryVitePlugin({
            org: SENTRY_ORG_NAME,
            project: SENTRY_PROJECT_NAME,
            authToken: process.env.SENTRY_AUTH_TOKEN,
            disable: !process.env.SENTRY_AUTH_TOKEN,
            telemetry: false,
        }),
    ],
    define: {
        COMMIT_REV: JSON.stringify(COMMIT_REV),
    },
    build: {
        sourcemap: true,
    },
});
