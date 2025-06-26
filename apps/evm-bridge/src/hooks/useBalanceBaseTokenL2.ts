import { useQuery } from '@tanstack/react-query';
import { useEvmRpcClient } from '../contexts';

export function useBalanceBaseTokenL2(address: string) {
    const { evmRpcClient } = useEvmRpcClient();

    return useQuery({
        queryKey: ['anchor-balance-base-token', address, evmRpcClient?.baseUrl],
        queryFn: async () => {
            if (!evmRpcClient?.baseUrl) return {};

            return await evmRpcClient.getBalanceBaseToken(address);
        },
        enabled: !!address && !!evmRpcClient?.baseUrl,
        staleTime: 1000 * 60 * 5,
    });
}
