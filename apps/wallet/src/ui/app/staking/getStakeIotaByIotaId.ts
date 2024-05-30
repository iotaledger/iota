// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type DelegatedStake } from '@mysten/iota.js/client';

// Get Stake IOTA by stakeIotaId
export const getStakeIotaByIotaId = (allDelegation: DelegatedStake[], stakeIotaId?: string | null) => {
	return (
		allDelegation.reduce((acc, curr) => {
			const total = BigInt(
				curr.stakes.find(({ stakedIotaId }) => stakedIotaId === stakeIotaId)?.principal || 0,
			);
			return total + acc;
		}, 0n) || 0n
	);
};
