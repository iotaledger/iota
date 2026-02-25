// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, test } from 'vitest';

import { GraphQLWebSocketClient } from '../../src/graphql/graphql-websocket-client';

const DEVNET_WS_URL = 'wss://graphql.devnet.iota.cafe/subscriptions';

const SUBSCRIPTION_TIMEOUT = 15_000;

describe('GraphQLWebSocketClient E2E (devnet)', () => {
    let client: GraphQLWebSocketClient | null = null;

    afterEach(() => {
        client?.close();
        client = null;
    });

    test(
        'connects to devnet and completes handshake',
        async () => {
            client = new GraphQLWebSocketClient(DEVNET_WS_URL);

            const unsub = await client.subscribe({
                query: `subscription { events { ... on Event { json } ... on Lagged { count } } }`,
                onMessage: () => {},
            });

            expect(typeof unsub).toBe('function');

            const result = await unsub();
            expect(result).toBe(true);
        },
        SUBSCRIPTION_TIMEOUT,
    );

    test(
        'receives events from devnet (or unsubscribes cleanly)',
        async () => {
            client = new GraphQLWebSocketClient(DEVNET_WS_URL);

            const messages: unknown[] = [];
            const errors: unknown[] = [];

            const unsub = await client.subscribe({
                query: `subscription { events { ... on Event { json bcs timestamp type { repr } } ... on Lagged { count } } }`,
                onMessage: (data: unknown) => {
                    messages.push(data);
                },
                onError: (errs: unknown) => {
                    errors.push(errs);
                },
            });

            await new Promise((resolve) => setTimeout(resolve, 5_000));

            const result = await unsub();
            expect(result).toBe(true);

            if (messages.length > 0) {
                const first = messages[0] as { events: { __typename: string } };
                expect(first).toHaveProperty('events');
                expect(first.events.__typename).toMatch(/^(Event|Lagged)$/);
            }

            expect(errors).toHaveLength(0);
        },
        SUBSCRIPTION_TIMEOUT,
    );

    test(
        'subscribes to transactions on devnet',
        async () => {
            client = new GraphQLWebSocketClient(DEVNET_WS_URL);

            const unsub = await client.subscribe({
                query: `subscription { transactions { ... on TransactionBlock { digest } ... on Lagged { count } } }`,
                onMessage: () => {},
            });

            expect(typeof unsub).toBe('function');

            const result = await unsub();
            expect(result).toBe(true);
        },
        SUBSCRIPTION_TIMEOUT,
    );
});
