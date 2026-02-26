// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { RequestInspector } from '@iota/iota-sdk/client';
import * as Sentry from '@sentry/react';

export function createSentryRequestInspector(url: string): RequestInspector {
    return async (input, executeRequest) => {
        return Sentry.startSpan(
            {
                name: input.method,
                op: 'http.rpc-request',
                data: input.params,
                tags: {
                    url,
                },
            },
            async (span) => {
                try {
                    const res = await executeRequest();
                    const status: Sentry.SpanStatusType = 'ok';
                    span?.setStatus(status);
                    return res;
                } catch (e) {
                    const status: Sentry.SpanStatusType = 'internal_error';
                    span?.setStatus(status);
                    throw e;
                } finally {
                    span?.end();
                }
            },
        );
    };
}
