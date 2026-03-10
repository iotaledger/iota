// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export const IS_PROD =
    process.env.BUILD_ENV === 'production' || process.env.NEXT_PUBLIC_BUILD_ENV === 'production';
export const IS_SENTRY_ENABLED = process.env.NEXT_PUBLIC_SENTRY_ENABLED === 'true';

export const SENTRY_DSN = IS_SENTRY_ENABLED
    ? IS_PROD
        ? 'https://880b00b12c5d12e3fbabfc2f9dc83344@o4508279186718720.ingest.de.sentry.io/4510205355360337'
        : 'https://de5a2b44cc967804379ecefe61281ccb@o4508279186718720.ingest.de.sentry.io/4510205404643408'
    : undefined;

export const SENTRY_PROJECT_NAME = IS_PROD ? 'iota-names-dapp' : 'iota-names-dapp-dev';
export const SENTRY_ORG_NAME = 'iota-foundation-eu';

export const SENTRY_IGNORE_ERRORS = ['Failed to connect to MetaMask', 'Rejected from user'];
