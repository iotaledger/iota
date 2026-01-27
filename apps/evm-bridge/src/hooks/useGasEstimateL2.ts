// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import { buildDepositL2Parameters } from '../lib/utils';
import { L2Chain } from '../config';
import { useAccount, usePublicClient } from 'wagmi';
import { iscAbi } from '../lib/constants';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

interface UseL2GasEstimateProps {
    address: string;
    amount: bigint;
    coinType?: string;
    refetchInterval?: number;
}

export function useGasEstimateL2({
    address,
    amount,
    coinType = IOTA_TYPE_ARG,
    refetchInterval,
}: UseL2GasEstimateProps) {
    const layer2Account = useAccount();
    const iscContractAddress = (layer2Account?.chain as L2Chain)?.iscContractAddress;
    const client = usePublicClient();

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: [
            'l2-deposit-transaction-gas-estimate',
            address,
            iscContractAddress,
            amount.toString(),
            coinType,
        ],
        async queryFn() {
            if (!address || !amount || !iscContractAddress) {
                return null;
            }

            const params = buildDepositL2Parameters(address, amount, coinType);

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
        refetchInterval,
    });
}
