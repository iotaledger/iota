// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as Sentry from '@sentry/nextjs';

export async function register() {
    // Only client is needed
}

export const onRequestError = Sentry.captureRequestError;
export const captureException = Sentry.captureException;
