// This file configures the initialization of Sentry on the client.
// The config you add here will be used whenever a users loads a page in their browser.
// https://docs.sentry.io/platforms/javascript/guides/nextjs/

import * as Sentry from '@sentry/nextjs';
import { IS_PROD, SENTRY_DSN } from './sentry.common.config.mjs';

Sentry.init({
    enabled: IS_PROD,
    dsn: SENTRY_DSN,

    // Define how likely traces are sampled. Adjust this value in production, or use tracesSampler for greater control.
    tracesSampleRate: 0, // Server is not traced

    // Setting this option to true will print useful information to the console while you're setting up Sentry.
    debug: false,
});
