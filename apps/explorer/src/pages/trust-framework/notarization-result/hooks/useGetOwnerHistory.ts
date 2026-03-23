// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useInfiniteQuery } from '@tanstack/react-query';
import { type ObjectOwner, type PaginatedTransactionResponse } from '@iota/iota-sdk/client';
import { useMemo } from 'react';
import { getOwnerAddress, getOwnerType } from '../../objectOwnerHelper';
import { normalizeIotaObjectId } from '@iota/iota-sdk/utils';

const OWNER_HISTORY_LIMIT = 5;

export interface OwnerEntry {
    ownerAddress: string;
    ownerType: string;
    transactionDigest: string;
    timestampMs: string | null;
}

/**
 * Fetches and processes the ownership history of a specified object ID.
 * It queries for transaction blocks that changed the object and extracts the owner from each relevant transaction.
 * It ensures that each owner is listed in descending order when the transfer happens, even if they appear in multiple transactions.
 *
 * For example if we have the following transactions:
 * tx 5 - owner B
 * tx 4 - owner B
 * tx 3 - owner A
 * tx 2 - owner A
 * tx 1 - owner C
 *
 * We have the following owner result list:
 * owner B
 * owner A
 * owner C
 *
 * @param objectId The object ID to get the owner history for.
 * @returns An object containing the processed list of owners and the state of the infinite query.
 */
export function useGetOwnerHistory(objectId: string) {
    const client = useIotaClient();

    const query = useInfiniteQuery<PaginatedTransactionResponse, Error>({
        queryKey: ['get-owner-history', objectId],
        queryFn: async ({ pageParam }) =>
            await client.queryTransactionBlocks({
                filter: { ChangedObject: objectId },
                cursor: pageParam as string | null,
                order: 'descending',
                limit: OWNER_HISTORY_LIMIT,
                options: {
                    showObjectChanges: true,
                    showInput: true,
                },
            }),
        initialPageParam: null,
        getNextPageParam: ({ hasNextPage, nextCursor }) => (hasNextPage ? nextCursor : null),
        staleTime: 10 * 1000,
        retry: false,
        enabled: !!objectId,
    });

    const owners = useMemo(() => {
        if (!query.data) return [];

        const entries: OwnerEntry[] = [];
        let currentOwnerAddress = null;

        for (const page of query.data.pages) {
            for (const tx of page.data) {
                if (tx.errors && tx.errors.length > 0) {
                    // Skip failed tx
                    continue;
                }

                if (!tx.confirmedLocalExecution) {
                    // NOTE: How to assert finality of a tx?
                    console.warn(`Transaction ${tx.digest} is not yet locally confirmed.`);
                }

                const change = tx.objectChanges?.find(
                    (c) =>
                        c.type !== 'published' &&
                        c.objectId === normalizeIotaObjectId(objectId) &&
                        (c.type === 'mutated' ||
                            c.type === 'created' ||
                            c.type === 'transferred') &&
                        // NOTE: Should I consider the package ID match here?
                        c.objectType.includes('::notarization::'),
                );

                if (!change) continue;

                const owner: ObjectOwner | undefined =
                    change.type === 'transferred'
                        ? change.recipient
                        : change.type === 'mutated' || change.type === 'created'
                          ? change.owner
                          : undefined;

                const ownerType = getOwnerType(owner);
                const ownerAddress = getOwnerAddress(owner, objectId);

                if (!ownerAddress || currentOwnerAddress === ownerAddress) continue;
                currentOwnerAddress = ownerAddress;

                entries.push({
                    ownerAddress,
                    ownerType,
                    transactionDigest: tx.digest,
                    timestampMs: tx.timestampMs ?? null,
                });
            }
        }

        return entries;
    }, [query.data, objectId]);

    return {
        owners,
        isPending: query.isPending,
        isError: query.isError,
        hasNextPage: query.hasNextPage,
        isFetchingNextPage: query.isFetchingNextPage,
        fetchNextPage: query.fetchNextPage,
    };
}
