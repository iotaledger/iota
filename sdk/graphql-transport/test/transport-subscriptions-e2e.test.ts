// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, test } from 'vitest';

import { getDefaultNetwork, getNetwork } from '../../typescript/src/client/network';
import { IotaClientGraphQLTransport } from '../src/transport';

const network = getDefaultNetwork();
const graphqlUrl = getNetwork(network).graphql;
if (!graphqlUrl) {
    throw new Error(
        `Missing GraphQL URL for ${network}. Ensure IOTA_NETWORKS env var is configured.`,
    );
}

const SUBSCRIPTION_TIMEOUT = 15_000;

describe(`IotaClientGraphQLTransport Subscriptions E2E (${network})`, () => {
    let transport: IotaClientGraphQLTransport | null = null;

    afterEach(() => {
        transport = null;
    });

    test(
        'subscribes to events via GraphQL WS',
        async () => {
            transport = new IotaClientGraphQLTransport({
                url: graphqlUrl,
            });

            const messages: unknown[] = [];

            const unsub = await transport.subscribe({
                method: 'iotax_subscribeEvent',
                unsubscribe: 'iotax_unsubscribeEvent',
                params: [{}],
                onMessage: (event: unknown) => {
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
        'subscribes to events with MoveModule filter',
        async () => {
            transport = new IotaClientGraphQLTransport({
                url: graphqlUrl,
            });

            const messages: unknown[] = [];

            const unsub = await transport.subscribe({
                method: 'iotax_subscribeEvent',
                unsubscribe: 'iotax_unsubscribeEvent',
                params: [{ Package: '0x3' }],
                onMessage: (event: unknown) => {
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
        'subscribes to transactions via GraphQL WS',
        async () => {
            transport = new IotaClientGraphQLTransport({
                url: graphqlUrl,
            });

            const messages: unknown[] = [];

            const unsub = await transport.subscribe({
                method: 'iotax_subscribeTransaction',
                unsubscribe: 'iotax_unsubscribeTransaction',
                params: [{}],
                onMessage: (tx: unknown) => {
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
        'supports AbortSignal for subscriptions',
        async () => {
            transport = new IotaClientGraphQLTransport({
                url: graphqlUrl,
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
