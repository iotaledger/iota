// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient, useIotaClientQuery } from '@iota/dapp-kit';
import type { IotaClient, LatestIotaSystemStateSummary } from '@iota/iota-sdk/client';
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

const getResultsForAddress = async (client: IotaClient, query: string): Promise<Results | null> => {
    const normalized = normalizeIotaObjectId(query);
    if (!isValidIotaAddress(normalized) || isGenesisLibAddress(normalized)) return null;

    const fromOrTo = await client.queryTransactionBlocks({
        filter: { FromOrToAddress: { addr: normalized } },
        limit: 1,
    });

    // Note: we need to query owned objects separately
    // because genesis addresses might not be involved in any transaction yet.
    let ownedObjects = [];
    if (!fromOrTo.data?.length) {
        const response = await client.getOwnedObjects({ owner: normalized, limit: 1 });
        ownedObjects = response.data;
    }

    if (!fromOrTo.data?.length && !ownedObjects?.length) return null;

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
    systemStateSummary: LatestIotaSystemStateSummary | null,
    query: string,
): Promise<Results | null> => {
    const normalized = normalizeIotaObjectId(query);
    if (
        (!isValidIotaAddress(normalized) && !isValidIotaObjectId(normalized)) ||
        !systemStateSummary
    )
        return null;

    // find validator by pool id or iota address
    const validator = systemStateSummary.activeValidators?.find(
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
    const { data: systemStateSummary } = useIotaClientQuery('getLatestIotaSystemState');

    return useQuery<Results, Error>({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['search', query],
        queryFn: async () => {
            const results = (
                await Promise.allSettled([
                    getResultsForTransaction(client, query),
                    getResultsForCheckpoint(client, query),
                    getResultsForAddress(client, query),
                    getResultsForObject(client, query),
                    getResultsForValidatorByPoolIdOrIotaAddress(systemStateSummary || null, query),
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
