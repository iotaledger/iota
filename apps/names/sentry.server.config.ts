// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// This file configures the initialization of Sentry on the server.
// The config you add here will be used whenever the server handles a request.
// https://docs.sentry.io/platforms/javascript/guides/nextjs/

import * as Sentry from '@sentry/nextjs';

import { IS_SENTRY_ENABLED, SENTRY_DSN, SENTRY_IGNORE_ERRORS } from './sentry.common.config.mjs';

Sentry.init({
    enabled: IS_SENTRY_ENABLED && Boolean(SENTRY_DSN),
    dsn: SENTRY_DSN,

    // Define how likely traces are sampled. Adjust this value in production, or use tracesSampler for greater control.
    tracesSampleRate: 0, // Server is not traced

    // Setting this option to true will print useful information to the console while you're setting up Sentry.
    debug: false,
    ignoreErrors: SENTRY_IGNORE_ERRORS,
});
