// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

import { roundFloat } from '../utils/roundFloat';

// recentEpochRewards is list of the last 30 epoch rewards for a specific validator
// APY_e = (epoch_rewards / stake) * 365
// APY_e_30rollingaverage = average(APY_e,APY_e-1,…,APY_e-29);

const DEFAULT_APY_DECIMALS = 2;

export interface ApyByValidator {
    [validatorAddress: string]: {
        apy: number;
        isApyApproxZero: boolean;
    };
}
// For small APY, show ~0% instead of 0%
// If APY falls below 0.001, show ~0% instead of 0% since we round to 2 decimal places
const MINIMUM_THRESHOLD = 0.001;

export function useGetValidatorsApy() {
    const client = useIotaClient();
    return useQuery({
        queryKey: ['get-rolling-average-apys'],
        queryFn: () => client.getValidatorsApy(),
        select: (validatorApys) => {
            return validatorApys?.apys.reduce((acc, { apy, address }) => {
                acc[address] = {
                    apy: roundFloat(apy * 100, DEFAULT_APY_DECIMALS),
                    isApyApproxZero: apy < MINIMUM_THRESHOLD,
                };
                return acc;
            }, {} as ApyByValidator);
        },
    });
}
