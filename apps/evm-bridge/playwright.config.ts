import { defineConfig, devices } from '@playwright/test';

/**
 * See https://playwright.dev/docs/test-configuration.
 */
export default defineConfig({
    testDir: './tests',
    fullyParallel: true,
    forbidOnly: !!process.env.CI,
    retries: process.env.CI ? 1 : 0,
    workers: process.env.CI ? 2 : undefined,
    reporter: 'html',
    expect: {
        timeout: 10_000,
    },
    use: {
        /* Base URL to use in actions like `await page.goto('/')`. */
        baseURL: 'http://localhost:4173',
        trace: 'on-first-retry',
    },
    projects: [
        {
            name: 'chromium',
            use: {
                ...devices['Desktop Chrome'],
                userAgent: 'Playwright',
                // Remove after we don't need copy address anymore (tests/utils/auth.ts > line with clipboard)
                contextOptions: {
                    permissions: ['clipboard-read'],
                },
            },
        },
    ],
    webServer: [
        {
            cwd: './',
            command: 'pnpm run preview',
            port: 4173,
            timeout: 30 * 1000,
            reuseExistingServer: !process.env.CI,
        },
    ],
});
