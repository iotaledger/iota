// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isIotaNSName, useIotaNSEnabled } from '@iota/core';
import { useIotaClientQuery, useIotaClient } from '@iota/dapp-kit';
import { type IotaClient, type IotaSystemStateSummary } from '@iota/iota-sdk/client';
import {
    isValidTransactionDigest,
    isValidIotaAddress,
    isValidIotaObjectId,
    normalizeIotaObjectId,
} from '@iota/iota-sdk/utils';
import { type UseQueryResult, useQuery } from '@tanstack/react-query';

const isGenesisLibAddress = (value: string): boolean => /^(0x|0X)0{0,39}[12]$/.test(value);

type Results = { id: string; label: string; type: string }[];

const getResultsForTransaction = async (
    client: IotaClient,
    query: string,
): Promise<Results | null> => {
    if (!isValidTransactionDigest(query)) return null;
    const txdata = await client.getTransactionBlock({ digest: query });
    return [
        {
            id: txdata.digest,
            label: txdata.digest,
            type: 'transaction',
        },
    ];
};

const getResultsForObject = async (client: IotaClient, query: string): Promise<Results | null> => {
    const normalized = normalizeIotaObjectId(query);
    if (!isValidIotaObjectId(normalized)) return null;

    const { data, error } = await client.getObject({ id: normalized });
    if (!data || error) return null;

    return [
        {
            id: data.objectId,
            label: data.objectId,
            type: 'object',
        },
    ];
};

const getResultsForCheckpoint = async (
    client: IotaClient,
    query: string,
): Promise<Results | null> => {
    // Checkpoint digests have the same format as transaction digests:
    if (!isValidTransactionDigest(query)) return null;

    const { digest } = await client.getCheckpoint({ id: query });
    if (!digest) return null;

    return [
        {
            id: digest,
            label: digest,
            type: 'checkpoint',
        },
    ];
};

const getResultsForAddress = async (
    client: IotaClient,
    query: string,
    iotaNSEnabled: boolean,
): Promise<Results | null> => {
    if (iotaNSEnabled && isIotaNSName(query)) {
        const resolved = await client.resolveNameServiceAddress({ name: query.toLowerCase() });
        if (!resolved) return null;
        return [
            {
                id: resolved,
                label: resolved,
                type: 'address',
            },
        ];
    }

    const normalized = normalizeIotaObjectId(query);
    if (!isValidIotaAddress(normalized) || isGenesisLibAddress(normalized)) return null;

    const [from, to] = await Promise.all([
        client.queryTransactionBlocks({
            filter: { FromAddress: normalized },
            limit: 1,
        }),
        client.queryTransactionBlocks({
            filter: { ToAddress: normalized },
            limit: 1,
        }),
    ]);

    if (!from.data?.length && !to.data?.length) return null;

    return [
        {
            id: normalized,
            label: normalized,
            type: 'address',
        },
    ];
};

// Query for validator by pool id or iota address.
const getResultsForValidatorByPoolIdOrIotaAddress = async (
    systemStateSummery: IotaSystemStateSummary | null,
    query: string,
): Promise<Results | null> => {
    const normalized = normalizeIotaObjectId(query);
    if (
        (!isValidIotaAddress(normalized) && !isValidIotaObjectId(normalized)) ||
        !systemStateSummery
    )
        return null;

    // find validator by pool id or iota address
    const validator = systemStateSummery.activeValidators?.find(
        ({ stakingPoolId, iotaAddress }) => stakingPoolId === normalized || iotaAddress === query,
    );

    if (!validator) return null;

    return [
        {
            id: validator.iotaAddress || validator.stakingPoolId,
            label: normalized,
            type: 'validator',
        },
    ];
};

export function useSearch(query: string): UseQueryResult<Results, Error> {
    const client = useIotaClient();
    const { data: systemStateSummery } = useIotaClientQuery('getLatestIotaSystemState');
    const iotaNSEnabled = useIotaNSEnabled();

    return useQuery<Results, Error>({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['search', query],
        queryFn: async () => {
            const results = (
                await Promise.allSettled([
                    getResultsForTransaction(client, query),
                    getResultsForCheckpoint(client, query),
                    getResultsForAddress(client, query, iotaNSEnabled),
                    getResultsForObject(client, query),
                    getResultsForValidatorByPoolIdOrIotaAddress(systemStateSummery || null, query),
                ])
            ).filter(
                (r) => r.status === 'fulfilled' && r.value,
            ) as PromiseFulfilledResult<Results>[];

            return results.map(({ value }) => value).flat();
        },
        enabled: !!query,
        gcTime: 10000,
    });
}
