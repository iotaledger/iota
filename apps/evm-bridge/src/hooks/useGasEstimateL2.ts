import { useQuery } from '@tanstack/react-query';
import { buildDepositL2Parameters } from '../lib/utils';
import { L2Chain } from '../config';
import { useAccount, usePublicClient } from 'wagmi';
import { iscAbi } from '../lib/constants';

interface UseL2GasEstimateProps {
    address: string;
    amount: string;
}

export function useGasEstimateL2({ address, amount }: UseL2GasEstimateProps) {
    const layer2Account = useAccount();
    const iscContractAddress = (layer2Account?.chain as L2Chain)?.iscContractAddress;
    const client = usePublicClient();

    return useQuery({
        queryKey: ['l2-deposit-transaction-gas-estimate', address, iscContractAddress, amount],
        async queryFn() {
            if (!address || !amount || !iscContractAddress) {
                return null;
            }
            const params = buildDepositL2Parameters(address, amount);
            const gas = await client?.estimateContractGas({
                address: iscContractAddress,
                abi: iscAbi,
                functionName: 'transferToL1',
                args: params,
                account: layer2Account.address,
            });

            let gasPrice = await client?.getGasPrice();
            if (!gasPrice) {
                gasPrice = 10000000000n;
            }
            return gas ? gas * gasPrice : null;
        },
        refetchInterval: 2000,
    });
}
