import { useQuery } from '@tanstack/react-query';
import { useEvmRpcClient } from '../contexts';
import { CoinBalance } from '@iota/iota-sdk/client';
import { AssetsResponse } from '@iota/isc-sdk';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

export function useGetAllBalancesL2(address: string) {
    const { evmRpcClient } = useEvmRpcClient();

    return useQuery({
        queryKey: ['anchor-balance-base-token', address, evmRpcClient?.baseUrl],
        queryFn: async () => {
            if (!evmRpcClient?.baseUrl || !address) {
                // Return a properly typed empty AssetsResponse
                return {
                    baseTokens: '0',
                    nativeTokens: [],
                } as AssetsResponse;
            }

            return await evmRpcClient.getBalanceBaseToken(address);
        },
        select: (data: AssetsResponse): CoinBalance[] => {
            if (!data) return [];

            const coinBalances: Array<{
                coinType: string;
                coinObjectCount: number;
                totalBalance: string;
            }> = [];

            // Add base token (IOTA) if it exists
            if (data.baseTokens !== undefined && data.baseTokens !== null) {
                coinBalances.push({
                    coinType: IOTA_TYPE_ARG,
                    coinObjectCount: 1,
                    totalBalance: data.baseTokens,
                });
            }

            // Add all native tokens if they exist
            if (data.nativeTokens && Array.isArray(data.nativeTokens)) {
                data.nativeTokens.forEach((token) => {
                    if (token.coinType && token.balance) {
                        coinBalances.push({
                            coinType: token.coinType,
                            coinObjectCount: 1,
                            totalBalance: token.balance,
                        });
                    }
                });
            }

            return coinBalances;
        },
        enabled: !!address && !!evmRpcClient?.baseUrl,
        staleTime: 1000 * 60 * 5,
    });
}
