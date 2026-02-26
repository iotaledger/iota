// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, test } from 'vitest';

import { getDefaultNetwork, getNetwork } from '../../typescript/src/client/network';
import { GraphQLWebSocketClient } from '../src/graphql-websocket-client';

const network = getDefaultNetwork();
const graphqlUrl = getNetwork(network).graphql;
if (!graphqlUrl) {
    throw new Error(
        `Missing GraphQL URL for ${network}. Ensure IOTA_NETWORKS env var is configured.`,
    );
}
const subscriptionUrl = graphqlUrl.replace(/\/?$/, '/subscriptions');

const SUBSCRIPTION_TIMEOUT = 15_000;

describe(`GraphQLWebSocketClient E2E (${network})`, () => {
    let client: GraphQLWebSocketClient | null = null;

    afterEach(() => {
        client?.close();
        client = null;
    });

    test(
        'connects to network and completes handshake',
        async () => {
            client = new GraphQLWebSocketClient(subscriptionUrl);

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
        'receives events from network (or unsubscribes cleanly)',
        async () => {
            client = new GraphQLWebSocketClient(subscriptionUrl);

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
        'subscribes to transactions on network',
        async () => {
            client = new GraphQLWebSocketClient(subscriptionUrl);

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
