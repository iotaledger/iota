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
        let scope = new Sentry.Scope();
        scope.setAttribute('params', input.params);
        scope.setTags({ url: this.url });
        return Sentry.startSpan(
            {
                name: input.method,
                op: 'http.rpc-request',
                scope,
            },
            async (span) => {
                try {
                    const res = await handler();
                    span?.setStatus({ code: 1 });
                    return res;
                } catch (e) {
                    span?.setStatus({ code: 2 });
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
