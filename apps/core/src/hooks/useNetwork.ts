import { useIotaClientContext } from "@iota/dapp-kit";

export function useNetwork(): string {
    const ctx = useIotaClientContext();
    console.log(ctx)
    return ctx.network
}