import { useIotaClientContext } from './useIotaClient.js';

export function useNetwork(): string {
    const iotaClientContext = useIotaClientContext();
    return iotaClientContext.network;
}
