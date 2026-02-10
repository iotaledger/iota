import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react-swc';
import dotenv from 'dotenv';
import { execSync } from 'child_process';
import { resolve } from 'path';

const SDK_ROOT = resolve(__dirname, '..', '..', 'sdk');
dotenv.config({ path: [resolve(SDK_ROOT, '.env'), resolve(SDK_ROOT, '.env.defaults')] });

const COMMIT_REV = execSync('git rev-parse HEAD').toString().trim().toString();

// https://vitejs.dev/config/
export default defineConfig({
    plugins: [react()],
    define: {
        COMMIT_REV: JSON.stringify(COMMIT_REV),
        'process.env.APPS_BACKEND': JSON.stringify(process.env.APPS_BACKEND ?? ''),
    },
});
