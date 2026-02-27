// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount } from '@iota/dapp-kit';
import { useBuildDepositTransactionL1 } from './useBuildDepositTransactionL1';
import { L1_BASE_GAS_BUDGET, L2_FROM_L1_GAS_BUDGET } from '@iota/isc-sdk';
import { MINIMUM_SEND_AMOUNT } from '../lib/constants';
import { IOTA_DECIMALS, parseAmount } from '@iota/iota-sdk/utils';
import { useBalance } from '@iota/core';

const GENERIC_EVM_ADDRESS = '0x1111111111111111111111111111111111111111';

export function useAvailableIotaBalanceL1(): {
    availableBalance: bigint;
    isLoading: boolean;
} {
    const layer1Account = useCurrentAccount();
    const { data: layer1BalanceData, isLoading: isLoadingL1 } = useBalance(
        layer1Account?.address as `0x${string}`,
    );

    const layer1TotalBalance = layer1BalanceData?.totalBalance
        ? BigInt(layer1BalanceData?.totalBalance)
        : 0n;

    // Estimate gas costs for Layer 1 transactions
    const minAmount = parseAmount(MINIMUM_SEND_AMOUNT.toString(), IOTA_DECIMALS) || 0n;
    const { data: minAmountDataL1, isLoading: isLoadingL1Transaction } =
        useBuildDepositTransactionL1({
            receivingAddress: GENERIC_EVM_ADDRESS,
            amount: minAmount,
        });

    const gasEstimationIOTA = BigInt(minAmountDataL1?.gasSummary?.budget || L1_BASE_GAS_BUDGET);

    // Check if the available amount is larger than the minimum send amount
    const isLayer1BalanceLargerThanMinimumSendAmount = layer1TotalBalance > minAmount;

    // Calculate the Layer 1 available balance, subtracting gas costs if the amount is valid
    const availableBalance = isLayer1BalanceLargerThanMinimumSendAmount
        ? layer1TotalBalance - gasEstimationIOTA - L2_FROM_L1_GAS_BUDGET
        : layer1TotalBalance;

    return {
        availableBalance,
        isLoading: isLoadingL1 || isLoadingL1Transaction,
    };
}
