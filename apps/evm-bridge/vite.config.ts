import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react-swc';
import { execSync } from 'child_process';
import pkg from './package.json';

process.env.VITE_APP_VERSION = pkg.version;

const COMMIT_REV = execSync('git rev-parse HEAD').toString().trim().toString();

// https://vitejs.dev/config/
export default defineConfig({
    plugins: [react()],
    define: {
        COMMIT_REV: JSON.stringify(COMMIT_REV),
    },
});
