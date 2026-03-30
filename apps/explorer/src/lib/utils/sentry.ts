// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as Sentry from '@sentry/react';
import { useEffect } from 'react';
import {
    createRoutesFromChildren,
    matchRoutes,
    useLocation,
    useNavigationType,
} from 'react-router-dom';

const IS_PROD = import.meta.env.VITE_BUILD_ENV === 'production';
const IS_SENTRY_ENABLED = import.meta.env.VITE_SENTRY_ENABLED === 'true';
const SENTRY_DSN = IS_SENTRY_ENABLED
    ? IS_PROD
        ? 'https://ce107602e4d122f0639332c7c43fdc08@o4508279186718720.ingest.de.sentry.io/4508279962140752'
        : 'https://c8085701fa2650fb2a090ed6aba6bc62@o4508279186718720.ingest.de.sentry.io/4508279963320400'
    : undefined;

// TODO: REMOVE AFTER ITS VALIDATED IN THE CI
console.log('----> import.meta.env.VITE_BUILD_ENV', import.meta.env.VITE_BUILD_ENV);
console.log('----> import.meta.env.VITE_SENTRY_ENABLED', import.meta.env.VITE_SENTRY_ENABLED);

export function initSentry() {
    Sentry.init({
        enabled: IS_SENTRY_ENABLED && Boolean(SENTRY_DSN),
        dsn: SENTRY_DSN,
        environment: import.meta.env.VITE_VERCEL_ENV,
        integrations: [
            Sentry.reactRouterV6BrowserTracingIntegration({
                useEffect,
                useLocation,
                useNavigationType,
                createRoutesFromChildren,
                matchRoutes,
            }),
        ],
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
