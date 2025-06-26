import { useIotaClientQuery } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

const DEFAULT_REFETCH_INTERVAL = 1000;
const DEFAULT_STALE_TIME = 5000;

export function useBalance(
    address: string,
    options: {
        coinType?: string;
        refetchInterval?: number | false;
        staleTime?: number;
    } = {},
    coinType: string = IOTA_TYPE_ARG,
) {
    const { refetchInterval = DEFAULT_REFETCH_INTERVAL, staleTime = DEFAULT_STALE_TIME } = options;
    return useIotaClientQuery(
        'getBalance',
        { coinType, owner: address },
        {
            enabled: !!address,
            refetchInterval,
            staleTime,
        },
    );
}
