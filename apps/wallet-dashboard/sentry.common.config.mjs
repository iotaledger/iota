// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export const IS_PROD =
    process.env.BUILD_ENV === 'production' || process.env.NEXT_PUBLIC_BUILD_ENV === 'production';
export const IS_SENTRY_ENABLED = process.env.NEXT_PUBLIC_SENTRY_ENABLED === 'true';

export const SENTRY_DSN = IS_SENTRY_ENABLED
    ? IS_PROD
        ? 'https://cb83626ca07d6cf66ca2f901cf53c051@o4508279186718720.ingest.de.sentry.io/4508647247249488'
        : 'https://ba5d6596291f12e88625e02eb942b742@o4508279186718720.ingest.de.sentry.io/4508647248691280'
    : undefined;

export const SENTRY_PROJECT_NAME = IS_PROD ? 'iota-wallet-dashboard' : 'iota-wallet-dashboard-dev';
export const SENTRY_ORG_NAME = 'iota-foundation-eu';
