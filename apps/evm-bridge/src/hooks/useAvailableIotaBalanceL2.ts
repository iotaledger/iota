// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAccount, useBalance } from 'wagmi';
import { useGasEstimateL2 } from './useGasEstimateL2';
import { MINIMUM_SEND_AMOUNT } from '../lib/constants';
import { formatEther } from 'viem';
import { IOTA_DECIMALS, parseAmount } from '@iota/iota-sdk/utils';

const GENERIC_IOTA_ADDRESS = '0x1111111111111111111111111111111111111111111111111111111111111111';

export function useAvailableIotaBalanceL2(): {
    availableBalance: bigint;
    isLoading: boolean;
} {
    const layer2Account = useAccount();

    // Fetch Layer 2 balance
    const { data: layer2BalanceData, isLoading: isLoadingL2 } = useBalance({
        address: layer2Account?.address as `0x${string}`,
        query: {
            refetchInterval: 2000,
        },
    });

    const layer2TotalBalance = layer2BalanceData?.value || 0n;

    const amount = parseAmount(MINIMUM_SEND_AMOUNT.toString(), IOTA_DECIMALS);
    const { data: gasEstimationData, isPending: isGasEstimationLoading } = useGasEstimateL2({
        address: GENERIC_IOTA_ADDRESS,
        amount,
    });

    const gasEstimation = gasEstimationData ?? 0n;

    // Calculate the Layer 2 available balance, subtracting gas costs if the amount is valid
    const availableBalance =
        layer2TotalBalance >= gasEstimation
            ? layer2TotalBalance - gasEstimation
            : layer2TotalBalance;

    // Convert the available balance to IOTA format because the balance is in wei (18 decimals)
    const formattedIota = formatEther(availableBalance);
    const availableBalanceInIota = parseAmount(formattedIota, IOTA_DECIMALS);

    return {
        availableBalance: availableBalanceInIota,
        isLoading: isLoadingL2 || isGasEstimationLoading,
    };
}
