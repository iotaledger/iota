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

const SENTRY_ENABLED = import.meta.env.PROD;
const SENTRY_SAMPLE_RATE = import.meta.env.VITE_SENTRY_SAMPLE_RATE
    ? parseFloat(import.meta.env.VITE_SENTRY_SAMPLE_RATE)
    : 0;

export function initSentry() {
    Sentry.init({
        enabled: SENTRY_ENABLED,
        dsn: import.meta.env.PROD
            ? 'https://e4e27ccfefe0d0dc5b1ccd4b28fd8ce7@o1010134.ingest.us.sentry.io/4508257079590912'
            : 'https://e2160952da44d9899bcb037bef100872@o1010134.ingest.us.sentry.io/4508256860045312',
        environment: import.meta.env.VITE_VERCEL_ENV,
        integrations: [
            new Sentry.BrowserTracing({
                routingInstrumentation: Sentry.reactRouterV6Instrumentation(
                    useEffect,
                    useLocation,
                    useNavigationType,
                    createRoutesFromChildren,
                    matchRoutes,
                ),
            }),
        ],
        tracesSampleRate: SENTRY_SAMPLE_RATE,
        beforeSend(event) {
            try {
                // Filter out any code from unknown sources:
                if (
                    !event.exception?.values?.[0].stacktrace ||
                    event.exception?.values?.[0].stacktrace?.frames?.[0].filename === '<anonymous>'
                ) {
                    return null;
                }
                // eslint-disable-next-line no-empty
            } catch (e) {}

            return event;
        },

        denyUrls: [
            // Chrome extensions
            /extensions\//i,
            /^chrome(?:-extension)?:\/\//i,
            /<anonymous>/,
        ],
        allowUrls: [/.*\.iota\.org/i, /.*\.iota\.cafe/i, /.*\.iotaledger\.net/i],
    });
}
