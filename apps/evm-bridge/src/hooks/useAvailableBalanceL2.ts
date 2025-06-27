import { useAccount, useBalance as useBalanceL2 } from 'wagmi';
import { useGasEstimateL2 } from './useGasEstimateL2';
import { MINIMUM_SEND_AMOUNT } from '../lib/constants';
import { formatEther } from 'viem';

const GENERIC_IOTA_ADDRESS = '0x1111111111111111111111111111111111111111111111111111111111111111';

export function useAvailableBalanceL2(): {
    availableBalance: bigint;
    isLoading: boolean;
    formattedAvailableBalance: string;
} {
    const layer2Account = useAccount();

    // Fetch Layer 2 balance
    const { data: layer2BalanceData, isLoading: isLoadingL2 } = useBalanceL2({
        address: layer2Account?.address as `0x${string}`,
        query: {
            refetchInterval: 2000, // Refetch Layer 2 balance every 2 seconds
        },
    });

    const layer2TotalBalance = layer2BalanceData?.value || 0n;

    const { data: gasEstimationData, isPending: isGasEstimationLoading } = useGasEstimateL2({
        address: GENERIC_IOTA_ADDRESS,
        amount: MINIMUM_SEND_AMOUNT.toString(),
    });

    const gasEstimation = gasEstimationData ?? 0n;

    // Calculate the Layer 2 available balance, subtracting gas costs if the amount is valid
    const availableBalance =
        layer2TotalBalance >= gasEstimation
            ? layer2TotalBalance - gasEstimation
            : layer2TotalBalance;

    return {
        availableBalance,
        isLoading: isLoadingL2 || isGasEstimationLoading,
        formattedAvailableBalance: formatEther(availableBalance),
    };
}
