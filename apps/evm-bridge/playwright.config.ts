import { defineConfig, devices } from '@playwright/test';

/**
 * See https://playwright.dev/docs/test-configuration.
 */
export default defineConfig({
    testDir: './tests',
    globalSetup: require.resolve('./tests/helpers/global-setup'),
    globalTeardown: require.resolve('./tests/helpers/global-teardown'),
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
            name: 'send-max-tests',
            testMatch: ['**/*sendMax*.spec.ts'], // Match any sendMax test files
            use: {
                ...devices['Desktop Chrome'],
                userAgent: 'Playwright',
                contextOptions: {
                    permissions: ['clipboard-read'],
                },
            },
        },
        {
            name: 'deposit-tests',
            testMatch: ['**/*deposit*.spec.ts'], // Match any deposit test files
            dependencies: ['send-max-tests'], // This makes it wait for send-max-tests
            use: {
                ...devices['Desktop Chrome'],
                userAgent: 'Playwright',
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
