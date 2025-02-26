import { useIotaClientContext } from '@iota/dapp-kit';

export function useNetwork(): string {
    const iotaClientContext = useIotaClientContext();
    return iotaClientContext.network;
}
