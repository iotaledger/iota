// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// This file configures the initialization of Sentry on the client.
// The added config here will be used whenever a users loads a page in their browser.
// https://docs.sentry.io/platforms/javascript/guides/nextjs/

import * as Sentry from '@sentry/nextjs';

import { IS_SENTRY_ENABLED, SENTRY_DSN, SENTRY_IGNORE_ERRORS } from '../sentry.common.config.mjs';

Sentry.init({
    enabled: IS_SENTRY_ENABLED && Boolean(SENTRY_DSN),
    dsn: SENTRY_DSN,

    // Define how likely traces are sampled. Adjust this value in production, or use tracesSampler for greater control.
    tracesSampleRate: 0.0025,
    // Enable logs to be sent to Sentry
    enableLogs: true,
    ignoreErrors: SENTRY_IGNORE_ERRORS,
});

export const onRouterTransitionStart = Sentry.captureRouterTransitionStart;
