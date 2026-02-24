// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, test } from 'vitest';

import { GraphQLWebSocketClient } from '../src/graphql-websocket-client';
import { IotaClientGraphQLTransport } from '../src/transport';

const DEVNET_GRAPHQL_URL = 'https://graphql.devnet.iota.cafe';
const DEVNET_WS_URL = 'wss://graphql.devnet.iota.cafe/subscriptions';

const SUBSCRIPTION_TIMEOUT = 15_000;

describe('GraphQL Subscriptions E2E (devnet)', () => {
    let client: GraphQLWebSocketClient | null = null;
    let transport: IotaClientGraphQLTransport | null = null;

    afterEach(() => {
        client?.close();
        client = null;
        transport = null;
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
                onMessage: (data) => {
                    messages.push(data);
                },
                onError: (errs) => {
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

    test(
        'IotaClientGraphQLTransport subscribes to events via GraphQL WS',
        async () => {
            transport = new IotaClientGraphQLTransport({
                url: DEVNET_GRAPHQL_URL,
            });

            const messages: unknown[] = [];

            const unsub = await transport.subscribe({
                method: 'iotax_subscribeEvent',
                unsubscribe: 'iotax_unsubscribeEvent',
                params: [{}],
                onMessage: (event) => {
                    messages.push(event);
                },
            });

            await new Promise((resolve) => setTimeout(resolve, 5_000));

            const result = await unsub();
            expect(result).toBe(true);

            if (messages.length > 0) {
                const event = messages[0] as Record<string, unknown>;
                expect(event).toHaveProperty('type');
                expect(event).toHaveProperty('parsedJson');
            }
        },
        SUBSCRIPTION_TIMEOUT,
    );

    test(
        'IotaClientGraphQLTransport subscribes to events with MoveModule filter',
        async () => {
            transport = new IotaClientGraphQLTransport({
                url: DEVNET_GRAPHQL_URL,
            });

            const messages: unknown[] = [];

            const unsub = await transport.subscribe({
                method: 'iotax_subscribeEvent',
                unsubscribe: 'iotax_unsubscribeEvent',
                params: [{ Package: '0x3' }],
                onMessage: (event) => {
                    messages.push(event);
                },
            });

            await new Promise((resolve) => setTimeout(resolve, 3_000));

            const result = await unsub();
            expect(result).toBe(true);
        },
        SUBSCRIPTION_TIMEOUT,
    );

    test(
        'IotaClientGraphQLTransport subscribes to transactions via GraphQL WS',
        async () => {
            transport = new IotaClientGraphQLTransport({
                url: DEVNET_GRAPHQL_URL,
            });

            const messages: unknown[] = [];

            const unsub = await transport.subscribe({
                method: 'iotax_subscribeTransaction',
                unsubscribe: 'iotax_unsubscribeTransaction',
                params: [{}], // Empty filter = all transactions
                onMessage: (tx) => {
                    messages.push(tx);
                },
            });

            await new Promise((resolve) => setTimeout(resolve, 5_000));

            const result = await unsub();
            expect(result).toBe(true);

            if (messages.length > 0) {
                const tx = messages[0] as Record<string, unknown>;
                expect(tx).toHaveProperty('digest');
            }
        },
        SUBSCRIPTION_TIMEOUT,
    );

    test(
        'IotaClientGraphQLTransport supports AbortSignal for subscriptions',
        async () => {
            transport = new IotaClientGraphQLTransport({
                url: DEVNET_GRAPHQL_URL,
            });

            const controller = new AbortController();

            const unsub = await transport.subscribe({
                method: 'iotax_subscribeEvent',
                unsubscribe: 'iotax_unsubscribeEvent',
                params: [{}],
                onMessage: () => {},
                signal: controller.signal,
            });

            await new Promise((resolve) => setTimeout(resolve, 1_000));
            controller.abort();

            const result = await unsub();
            expect(typeof result).toBe('boolean');
        },
        SUBSCRIPTION_TIMEOUT,
    );
});
