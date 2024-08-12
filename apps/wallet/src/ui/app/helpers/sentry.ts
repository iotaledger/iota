// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { growthbook } from '_src/ui/app/experimentation/feature-gating';
import * as Sentry from '@sentry/react';

import { getSentryConfig } from '_shared/sentry-config';

export default function initSentry() {
    Sentry.init(
        getSentryConfig({
            integrations: [new Sentry.BrowserTracing()],
            tracesSampler: () => {
                return growthbook.getFeatureValue('wallet-sentry-tracing', 0);
            },
        }),
    );
}
