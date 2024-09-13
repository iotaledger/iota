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
    : 1;

export function initSentry() {
    Sentry.init({
        enabled: SENTRY_ENABLED,
        dsn: import.meta.env.PROD
            ? import.meta.env.VITE_PROD_SENTRY_DSN
            : import.meta.env.VITE_DEV_SENTRY_DSN,
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
        allowUrls: [
            /.*\.iota\.io/i,
            /.*-iota-foundation\.vercel\.app/i,
            'explorer-topaz.vercel.app',
        ],
    });
}
