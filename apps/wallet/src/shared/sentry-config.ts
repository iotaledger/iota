// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type BrowserOptions } from '@sentry/browser';
import Browser from 'webextension-polyfill';

const WALLET_VERSION = Browser.runtime.getManifest().version;
const IS_PROD = process.env.NODE_ENV === 'production';

// Sentry dev hint: If you want to enable sentry in dev, you can tweak this value:
const ENABLE_SENTRY = IS_PROD;

const SENTRY_DSN = IS_PROD
    ? 'https://ca3888d99b4bb1bfd5e5466ba7cc2d8f@o1010134.ingest.us.sentry.io/4508256609697792'
    : 'https://d80ad35fe98bd767515050181efdec38@o1010134.ingest.us.sentry.io/4508256233848832';

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
