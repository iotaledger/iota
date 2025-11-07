// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAccount, useChainId, useWaitForTransactionReceipt, useWriteContract } from 'wagmi';
import { useEffect } from 'react';
import { DepositForm } from '../DepositForm';
import toast from 'react-hot-toast';
import { buildDepositL2Parameters } from '../../../lib/utils';
import { iscAbi, L2_USER_REJECTED_TX_ERROR_TEXT } from '../../../lib/constants';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useFormContext } from 'react-hook-form';
import { DepositFormData } from '../../../lib/schema/bridgeForm.schema';
import { L2Chain } from '../../../config';
import { getBalanceQueryKey } from 'wagmi/query';
import { useGasEstimateL2 } from '../../../hooks/useGasEstimateL2';
import { formatEther } from 'viem';
import { IOTA_DECIMALS, parseAmount } from '@iota/iota-sdk/utils';
import { useCoinMetadata } from '@iota/core';
import { useGetAllBalancesL2 } from '../../../hooks/useGetAllBalancesL2';
import { ampli } from '../../../shared/analytics';

export function DepositLayer2() {
    const queryClient = useQueryClient();
    const layer2Account = useAccount();
    const chainId = useChainId();
    const iscContractAddress = (layer2Account?.chain as L2Chain)?.iscContractAddress;

    const { refetch: refetchL2Balance } = useGetAllBalancesL2(
        layer2Account.address as `0x${string}`,
    );
    const { watch } = useFormContext<DepositFormData>();
    const { depositAmount, receivingAddress, coinType: selectedCoinType } = watch();

    const { data: coinMetadata } = useCoinMetadata(selectedCoinType);

    const amount = parseAmount(depositAmount, coinMetadata?.decimals ?? IOTA_DECIMALS) || BigInt(0);

    const { data: hash, writeContractAsync, isSuccess, isError, error } = useWriteContract({});

    const {
        isSuccess: isTransactionSuccess,
        isError: isTransactionError,
        error: transactionError,
    } = useWaitForTransactionReceipt({
        hash: hash,
    });

    const { data: gasEstimationEVM, isPending: isGasEstimationLoading } = useGasEstimateL2({
        address: receivingAddress,
        amount,
        coinType: selectedCoinType,
        refetchInterval: 2000,
    });

    useEffect(() => {
        if (isSuccess && hash) {
            toast('Withdraw transaction submitted!');
        }
    }, [isSuccess, hash]);

    useEffect(() => {
        if (isError && error) {
            if (import.meta.env.DEV) {
                console.error('Failed submitting transaction:', error.message);
            }

            if (error.message.startsWith(L2_USER_REJECTED_TX_ERROR_TEXT)) {
                toast.error('Transaction canceled by user.');
            } else {
                toast.error('Something went wrong while submitting withdraw transaction.');
            }
        }
    }, [isError, error]);

    useEffect(() => {
        if (isTransactionSuccess) {
            toast.success('Withdraw transaction confirmed! Your funds have been transferred.');
            const balanceQueryKey = getBalanceQueryKey({
                chainId,
                address: layer2Account.address,
            });
            queryClient.invalidateQueries({ queryKey: balanceQueryKey });
            refetchL2Balance();
        }
    }, [isTransactionSuccess]);

    useEffect(() => {
        if (isTransactionError && transactionError) {
            if (import.meta.env.DEV) {
                console.error('Error while waiting for transaction', transactionError.message);
            }
            toast.error('Unable to complete withdraw transaction.');
        }
    }, [isTransactionError, transactionError]);

    const { mutate: deposit, isPending: isTransactionLoading } = useMutation({
        mutationKey: [
            'l2-deposit-transaction',
            receivingAddress,
            depositAmount,
            iscContractAddress,
            chainId,
            selectedCoinType,
        ],
        async mutationFn() {
            if (!receivingAddress || !depositAmount || !iscContractAddress) {
                throw Error('Transaction is missing');
            }
            ampli.sentFromL2ToL1({
                amount: depositAmount,
                coinType: selectedCoinType,
            });
            const params = buildDepositL2Parameters(receivingAddress, amount, selectedCoinType);
            await writeContractAsync({
                abi: iscAbi,
                address: iscContractAddress,
                functionName: 'transferToL1',
                args: params,
                chainId,
            });
        },
    });

    return (
        <DepositForm
            deposit={deposit}
            isGasEstimationLoading={isGasEstimationLoading}
            isTransactionLoading={isTransactionLoading}
            gasEstimationEVM={formatEther(gasEstimationEVM || 0n)}
        />
    );
}
