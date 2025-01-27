// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaHTTPTransport } from '@iota/iota-sdk/client';
import * as Sentry from '@sentry/react';

const IGNORED_METHODS: string[] = [];

export class SentryHttpTransport extends IotaHTTPTransport {
    private url: string;
    constructor(url: string) {
        super({ url });
        this.url = url;
    }

    async withRequest<T>(input: { method: string; params: unknown[] }, handler: () => Promise<T>) {
        return Sentry.startSpan(
            {
                name: input.method,
                op: 'http.rpc-request',
                data: input.params,
                tags: {
                    url: this.url,
                },
            },
            async (span) => {
                try {
                    const res = await handler();
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
    }

    override async request<T>(input: { method: string; params: unknown[] }) {
        if (IGNORED_METHODS.includes(input.method)) {
            return super.request<T>(input);
        }

        return this.withRequest(input, () => super.request<T>(input));
    }
}
