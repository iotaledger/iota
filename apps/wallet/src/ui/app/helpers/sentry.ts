// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { growthbook } from '_src/ui/app/experimentation/featureGating';
import * as Sentry from '@sentry/react';
import { Feature } from '@iota/core';

import { getSentryConfig } from '../../../shared/sentryConfig';

export default function initSentry() {
    Sentry.init(
        getSentryConfig({
            integrations: [new Sentry.BrowserTracing()],
            tracesSampler: () => {
                return growthbook.getFeatureValue(Feature.WalletSentryTracing, 0);
            },
        }),
    );
}
