// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as Sentry from '@sentry/react';

const IS_PROD = import.meta.env.VITE_BUILD_ENV === 'production';
export const IS_SENTRY_ENABLED = import.meta.env.VITE_SENTRY_ENABLED === 'true';

const SENTRY_DSN = IS_SENTRY_ENABLED
    ? IS_PROD
        ? 'https://c81b9282933b63a064b73860d59d4ad5@o4508279186718720.ingest.de.sentry.io/4511065397264464'
        : 'https://6114d8f3c9c3d8fd4871709ddda99241@o4508279186718720.ingest.de.sentry.io/4511065404670032'
    : undefined;

export function initSentry() {
    Sentry.init({
        enabled: IS_SENTRY_ENABLED && Boolean(SENTRY_DSN),
        dsn: SENTRY_DSN,
        environment: import.meta.env.VITE_VERCEL_ENV,
        integrations: [Sentry.browserTracingIntegration()],
        tracesSampleRate: IS_PROD ? 0.0025 : 1.0,
        // Browser extensions
        denyUrls: [
            /extensions\//i,
            /^chrome(?:-extension)?:\/\//i,
            /^moz-extension:\/\//i,
            /^safari-(?:web-)?extension:\/\//i,
            /^edge:\/\//i,
            /<anonymous>/,
        ],
        allowUrls: [/.*\.iota\.org/i, /.*\.iota\.cafe/i, /.*\.iotaledger\.net/i],
    });
}
