import { useCurrentAccount } from '@iota/dapp-kit';
import { useBalance as useBalanceL1 } from './useBalance';
import { formatIOTAFromNanos, parseAmount } from '../lib/utils';
import { useBuildL1DepositTransaction } from './useBuildL1DepositTransaction';
import { L1_BASE_GAS_BUDGET, L2_FROM_L1_GAS_BUDGET } from '@iota/isc-sdk';
import { MINIMUM_SEND_AMOUNT } from '../lib/constants';
import { IOTA_DECIMALS } from '@iota/iota-sdk/utils';

const GENERIC_EVM_ADDRESS = '0x1111111111111111111111111111111111111111';

export function useAvailableBalanceL1(): {
    availableBalance: bigint;
    isLoading: boolean;
    formattedAvailableBalance: string;
} {
    const layer1Account = useCurrentAccount();

    const { data: layer1BalanceData, isLoading: isLoadingL1 } = useBalanceL1(
        layer1Account?.address as `0x${string}`,
    );

    const layer1TotalBalance = layer1BalanceData?.totalBalance
        ? BigInt(layer1BalanceData?.totalBalance)
        : 0n;

    // Estimate gas costs for Layer 1 transactions
    const { data: maxAmountDataL1, isLoading: isLoadingL1Transaction } =
        useBuildL1DepositTransaction({
            receivingAddress: GENERIC_EVM_ADDRESS,
            amount: layer1TotalBalance - L1_BASE_GAS_BUDGET,
        });

    const gasEstimationIOTA = BigInt(maxAmountDataL1?.gasSummary?.budget || 0);

    // Check if the available amount is larger than the minimum send amount
    const isLayer1BalanceLargerThanMinimumSendAmount =
        layer1TotalBalance > (parseAmount(MINIMUM_SEND_AMOUNT.toString(), IOTA_DECIMALS) ?? 0n);

    // Calculate the Layer 1 available balance, subtracting gas costs if the amount is valid
    const availableBalance = isLayer1BalanceLargerThanMinimumSendAmount
        ? layer1TotalBalance - gasEstimationIOTA - L2_FROM_L1_GAS_BUDGET
        : layer1TotalBalance;

    return {
        availableBalance,
        isLoading: isLoadingL1 || isLoadingL1Transaction,
        formattedAvailableBalance: formatIOTAFromNanos(availableBalance),
    };
}
