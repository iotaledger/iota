// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export const IS_PROD =
    import.meta.env.VITE_BUILD_ENV === 'production' ||
    import.meta.env.VITE_SENTRY_BUILD_ENV === 'production';

export const SENTRY_DSN = IS_PROD
    ? import.meta.env.VITE_SENTRY_DSN_PROD || ''
    : import.meta.env.VITE_SENTRY_DSN_DEV || '';

export const SENTRY_PROJECT_NAME = IS_PROD ? 'iota-evm-bridge' : 'iota-evm-bridge-dev';
export const SENTRY_ORG_NAME = 'iota-foundation-eu';
