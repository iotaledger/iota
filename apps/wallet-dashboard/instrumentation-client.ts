// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// This file configures the initialization of Sentry on the client.
// The config you add here will be used whenever a users loads a page in their browser.
// https://docs.sentry.io/platforms/javascript/guides/nextjs/

import * as Sentry from '@sentry/nextjs';
import { IS_PROD, IS_SENTRY_ENABLED, SENTRY_DSN } from './sentry.common.config.mjs';

Sentry.init({
    enabled: IS_SENTRY_ENABLED,
    dsn: SENTRY_DSN,

    // Define how likely traces are sampled. Adjust this value in production, or use tracesSampler for greater control.
    tracesSampleRate: IS_PROD ? 0.0025 : 1.0,

    // Setting this option to true will print useful information to the console while you're setting up Sentry.
    debug: !IS_PROD,
});

export const onRouterTransitionStart = Sentry.captureRouterTransitionStart;
