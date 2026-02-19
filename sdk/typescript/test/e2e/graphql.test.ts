// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { beforeAll, describe, expect, it, test } from 'vitest';

import { IotaGraphQLClient } from '../../src/graphql';
import { graphql } from '../../src/graphql/schemas/2025.2';
import { IotaClient } from '../../src/client/index.js';
import { Transaction } from '../../src/transactions/index.js';
import { setup, TestToolbox } from './utils/setup';

const LOCALNET_INDEXER = 'http:127.0.0.1:9124';

const queries = {
    getFirstTransactionBlock: graphql(`
        query getEpochs($limit: Int!) {
            transactionBlocks(first: $limit, filter: { atCheckpoint: 0 }) {
                pageInfo {
                    hasNextPage
                    hasPreviousPage
                    endCursor
                    startCursor
                }
                edges {
                    node {
                        kind {
                            __typename
                        }
                        gasInput {
                            gasBudget
                        }
                    }
                }
            }
        }
    `),
};

const client = new IotaGraphQLClient({
    url: 'http://127.0.0.1:9125',
    queries,
});

describe('GraphQL client', () => {
    it('executes predefined queries', async () => {
        const response = await client.execute('getFirstTransactionBlock', {
            variables: {
                limit: 1,
            },
        });

        expect(response.data?.transactionBlocks.edges[0].node.kind?.__typename).toEqual(
            'GenesisTransaction',
        );
    });

    it('executes inline queries', async () => {
        const response = await client.query({
            query: graphql(`
                query getEpochs($limit: Int!) {
                    transactionBlocks(first: $limit, filter: { atCheckpoint: 0 }) {
                        edges {
                            node {
                                kind {
                                    __typename
                                    ... on GenesisTransaction {
                                        objects(first: 1) {
                                            nodes {
                                                asMovePackage {
                                                    version
                                                    modules(first: 3) {
                                                        nodes {
                                                            name
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                gasInput {
                                    gasBudget
                                }
                            }
                        }
                    }
                }
            `),
            variables: {
                limit: 1,
            },
        });

        expect(response.data?.transactionBlocks.edges[0].node.kind?.__typename).toEqual(
            'GenesisTransaction',
        );

        expect(response).toEqual({
            data: {
                transactionBlocks: {
                    edges: [
                        {
                            node: {
                                kind: {
                                    __typename: 'GenesisTransaction',
                                    objects: {
                                        nodes: [
                                            {
                                                asMovePackage: {
                                                    version: 1,
                                                    modules: {
                                                        nodes: [
                                                            {
                                                                name: 'address',
                                                            },
                                                            {
                                                                name: 'ascii',
                                                            },
                                                            {
                                                                name: 'bcs',
                                                            },
                                                        ],
                                                    },
                                                },
                                            },
                                        ],
                                    },
                                },
                                gasInput: {
                                    gasBudget: '0',
                                },
                            },
                        },
                    ],
                },
            },
        });
    });
});

describe('GraphQL transactionBlocksByDigests', () => {
    let toolbox: TestToolbox;
    let transactionBlockDigest: string;
    let anotherTransactionBlockDigest: string;
    const rpcClient = new IotaClient({ url: LOCALNET_INDEXER });

    beforeAll(async () => {
        toolbox = await setup({ rpcURL: LOCALNET_INDEXER });

        // create a simple transaction
        const tx = new Transaction();
        const [coin] = tx.splitCoins(tx.gas, [1]);
        tx.transferObjects([coin], toolbox.address());
        const result = await toolbox.client.signAndExecuteTransaction({
            transaction: tx as never,
            signer: toolbox.keypair,
        });

        transactionBlockDigest = result.digest;

        await rpcClient.waitForTransaction({
            digest: transactionBlockDigest,
            waitMode: 'checkpoint',
        });

        // create another transaction
        const anotherTx = new Transaction();
        const [coins] = anotherTx.splitCoins(anotherTx.gas, [1]);
        anotherTx.transferObjects([coins], toolbox.address());
        const anotherResult = await toolbox.client.signAndExecuteTransaction({
            transaction: anotherTx as never,
            signer: toolbox.keypair,
        });

        anotherTransactionBlockDigest = anotherResult.digest;

        await rpcClient.waitForTransaction({
            digest: anotherTransactionBlockDigest,
            waitMode: 'checkpoint',
        });
    });

    const transactionBlocksByDigestsQuery = graphql(`
        query transactionBlocksByDigests($digests: [String!]!) {
            transactionBlocksByDigests(digests: $digests) {
                digest
            }
        }
    `);

    test('transactionBlocksByDigests - single digest', async () => {
        const result = await client.query({
            query: transactionBlocksByDigestsQuery,
            variables: { digests: [transactionBlockDigest] },
        });

        const txBlocks = result.data?.transactionBlocksByDigests;
        expect(txBlocks).toHaveLength(1);
        expect(txBlocks![0]).toBeTruthy();
        expect(txBlocks![0]?.digest).toBe(transactionBlockDigest);
    });

    test('transactionBlocksByDigests - multiple digests', async () => {
        const result = await client.query({
            query: transactionBlocksByDigestsQuery,
            variables: {
                digests: [transactionBlockDigest, anotherTransactionBlockDigest],
            },
        });

        const txBlocks = result.data?.transactionBlocksByDigests;
        expect(txBlocks).toHaveLength(2);
        expect(txBlocks![0]).toBeTruthy();
        expect(txBlocks![0]?.digest).toBe(transactionBlockDigest);
        expect(txBlocks![1]).toBeTruthy();
        expect(txBlocks![1]?.digest).toBe(anotherTransactionBlockDigest);
    });

    test('transactionBlocksByDigests - with non-existent digest', async () => {
        const nonExistentDigest = 'C6G8PsqwNpMqrK7ApwuQUvDgzkFcUaUy6Y5ycrAN2q3F';
        const result = await client.query({
            query: transactionBlocksByDigestsQuery,
            variables: { digests: [nonExistentDigest] },
        });

        const txBlocks = result.data?.transactionBlocksByDigests;
        expect(txBlocks).toHaveLength(1);
        expect(txBlocks![0]).toBeNull();
    });
});
