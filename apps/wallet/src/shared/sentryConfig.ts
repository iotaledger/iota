// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type BrowserOptions } from '@sentry/browser';
import Browser from 'webextension-polyfill';

const WALLET_VERSION = Browser.runtime.getManifest().version;
const IS_PROD = process.env.BUILD_ENV === 'production';

// Sentry dev hint: If you want to enable sentry in dev, you can tweak this value:
const ENABLE_SENTRY = IS_PROD;

const SENTRY_DSN = IS_PROD
    ? 'https://36e3e3c59a2e842034b2fc624103b72a@o4508279186718720.ingest.de.sentry.io/4508279958536272'
    : 'https://36c25e34e606cac787b1536348101976@o4508279186718720.ingest.de.sentry.io/4508279960895568';

export function getSentryConfig({
    integrations,
    tracesSampler,
}: Pick<BrowserOptions, 'integrations' | 'tracesSampler'>): BrowserOptions {
    return {
        enabled: ENABLE_SENTRY,
        dsn: SENTRY_DSN,
        integrations,
        release: WALLET_VERSION,
        tracesSampler: IS_PROD ? tracesSampler : () => 1,
        allowUrls: IS_PROD
            ? [
                  'nlmllpflpelpannpijhhnbhekpbpejch', // chrome rc
                  'iidjkmdceolghepehaaddojmnjnkkija', // chrome prod
              ]
            : undefined,
    };
}
