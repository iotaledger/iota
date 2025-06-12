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

export function DepositLayer2() {
    const queryClient = useQueryClient();
    const layer2Account = useAccount();
    const chainId = useChainId();
    const iscContractAddress = (layer2Account?.chain as L2Chain)?.iscContractAddress;

    const { watch } = useFormContext<DepositFormData>();
    const { depositAmount, receivingAddress } = watch();

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
        amount: depositAmount,
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
            const blanceQueryKey = getBalanceQueryKey({
                chainId,
                address: layer2Account.address,
            });
            queryClient.invalidateQueries({ queryKey: blanceQueryKey });
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
        ],
        async mutationFn() {
            if (!receivingAddress || !depositAmount || !iscContractAddress) {
                throw Error('Transaction is missing');
            }
            const params = buildDepositL2Parameters(receivingAddress, depositAmount);
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
