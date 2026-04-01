// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { appsBackendClient } from '_src/shared/experimentation/features';
import { getSentryConfig } from '_src/shared/sentryConfig';
import * as Sentry from '@sentry/browser';
import { Feature } from '@iota/core';

export function initSentry() {
    Sentry.addTracingExtensions();
    Sentry.init(
        getSentryConfig({
            tracesSampler: () => {
                return appsBackendClient.getFeatureValue(Feature.WalletSentryTracing, 0);
            },
        }),
    );
}

export const captureException = Sentry.captureException;
export const captureMessage = Sentry.captureMessage;
