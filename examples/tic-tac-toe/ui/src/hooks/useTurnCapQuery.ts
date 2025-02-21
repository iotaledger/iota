// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount, useIotaClient, useIotaClientContext } from '@iota/dapp-kit';
import { useQuery, useQueryClient, UseQueryResult } from '@tanstack/react-query';
import { useNetworkVariable } from 'config';

export type TurnCap = {
	id: { id: string };
	game: string;
};

export type UseTurnCapResult = UseQueryResult<TurnCap | null, Error>;
export type InvalidateTurnCapQuery = () => void;

/** Refetch TurnCaps every 5 seconds */
const REFETCH_INTERVAL = 5000;

/**
 * Hook to fetch the `TurnCap` owned by `address` associated with
 * `game`, if there is one.
 */
export function useTurnCapQuery(game?: string): [UseTurnCapResult, InvalidateTurnCapQuery] {
	const iotaClient = useIotaClient();
	const queryClient = useQueryClient();
	const ctx = useIotaClientContext();
	const packageId = useNetworkVariable('packageId');
	const account = useCurrentAccount();

	const queryKey = [ctx.network, 'turn-cap', account?.address, game];
	const response = useQuery({
		enabled: !!game,
		refetchInterval: REFETCH_INTERVAL,
		queryKey,
		queryFn: async () => {
			const owner = account?.address;
			if (!owner) {
				return null;
			}

			for (;;) {
				const resp = await iotaClient.getOwnedObjects({
					owner,
					filter: {
						StructType: `${packageId}::owned::TurnCap`,
					},
					options: {
						showContent: true,
					},
				});

				for (const obj of resp.data) {
					const content = obj.data?.content;
					if (content?.dataType !== 'moveObject') {
						continue;
					}

					const cap = content.fields as TurnCap;
					if (cap.game === game) {
						return cap;
					}
				}

				if (!resp.hasNextPage) {
					break;
				}
			}

			return null;
		},
	});

	const invalidate = async () => {
		await queryClient.invalidateQueries({ queryKey });
	};

	return [response, invalidate];
}
