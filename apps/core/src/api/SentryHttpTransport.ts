// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaHTTPTransport } from '@iota/iota-sdk/client';
import * as Sentry from '@sentry/react';

const IGNORED_METHODS = ['iotax_resolveNameServiceNames', 'iotax_resolveNameServiceAddresses'];

export class SentryHttpTransport extends IotaHTTPTransport {
    private url: string;
    constructor(url: string) {
        super({ url });
        this.url = url;
    }

    async withRequest<T>(input: { method: string; params: unknown[] }, handler: () => Promise<T>) {
        const transaction = Sentry.startTransaction({
            name: input.method,
            op: 'http.rpc-request',
            data: input.params,
            tags: {
                url: this.url,
            },
        });

        try {
            const res = await handler();
            const status: Sentry.SpanStatusType = 'ok';
            transaction.setStatus(status);
            return res;
        } catch (e) {
            const status: Sentry.SpanStatusType = 'internal_error';
            transaction.setStatus(status);
            throw e;
        } finally {
            transaction.finish();
        }
    }

    override async request<T>(input: { method: string; params: unknown[] }) {
        if (IGNORED_METHODS.includes(input.method)) {
            return super.request<T>(input);
        }

        return this.withRequest(input, () => super.request<T>(input));
    }
}
