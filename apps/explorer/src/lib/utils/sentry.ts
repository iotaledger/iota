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

const SENTRY_ENABLED = import.meta.env.VITE_BUILD_ENV === 'production';
const SENTRY_SAMPLE_RATE = import.meta.env.VITE_SENTRY_SAMPLE_RATE
    ? parseFloat(import.meta.env.VITE_SENTRY_SAMPLE_RATE)
    : 0;

export function initSentry() {
    Sentry.init({
        enabled: SENTRY_ENABLED,
        dsn: SENTRY_ENABLED
            ? 'https://ce107602e4d122f0639332c7c43fdc08@o4508279186718720.ingest.de.sentry.io/4508279962140752'
            : 'https://c8085701fa2650fb2a090ed6aba6bc62@o4508279186718720.ingest.de.sentry.io/4508279963320400',
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
