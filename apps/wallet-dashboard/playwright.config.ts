import { defineConfig, devices } from '@playwright/test';

/**
 * See https://playwright.dev/docs/test-configuration.
 */
export default defineConfig({
    testDir: './tests',
    /* Run tests in files in parallel */
    fullyParallel: true,
    /* Fail the build on CI if you accidentally left test.only in the source code. */
    forbidOnly: true,
    /* Retry on CI only */
    retries: 2,
    /* Opt out of parallel tests on CI. */
    workers: 1,
    /* Reporter to use. See https://playwright.dev/docs/test-reporters */
    reporter: 'html',
    /* Shared settings for all the projects below. See https://playwright.dev/docs/api/class-testoptions. */
    use: {
        /* Base URL to use in actions like `await page.goto('/')`. */
        baseURL: 'http://localhost:3000/',

        /* Collect trace when retrying the failed test. See https://playwright.dev/docs/trace-viewer */
        trace: 'on-first-retry',
        screenshot: 'only-on-failure',
    },
    /* Maximum time one test can run for. */
    timeout: 60 * 1000,
    expect: {
        /**
         * Maximum time expect() should wait for the condition to be met.
         * For example in `await expect(locator).toHaveText();`
         */
        timeout: 10_000,
    },

    projects: [
        {
            name: 'chromium',
            use: { ...devices['Desktop Chrome'], channel: 'chromium' },
        },
    ],

    webServer: [
        // Wallet-dashboard:
        {
            command: 'pnpm start',
            port: 3000,
            timeout: 120 * 1000,
            reuseExistingServer: true,
        },
        // Apps-backend:
        {
            command: 'cd ../apps-backend && pnpm run preview',
            port: 3003,
            timeout: 120 * 1000,
            reuseExistingServer: true,
        },
    ],
});
