// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount, useIotaClient, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { DepositForm } from '../DepositForm';
import toast from 'react-hot-toast';

import { L1_USER_REJECTED_TX_ERROR_TEXT } from '../../../lib/constants';
import { useBuildDepositTransactionL1 } from '../../../hooks/useBuildDepositTransactionL1';
import { useFormContext } from 'react-hook-form';
import { DepositFormData } from '../../../lib/schema/bridgeForm.schema';
import { L2_FROM_L1_GAS_BUDGET } from '@iota/isc-sdk';
import { CoinFormat, formatBalance, IOTA_DECIMALS, parseAmount } from '@iota/iota-sdk/utils';
import { useCoinMetadata, useGetAllCoins } from '@iota/core';
import { useGetAllBalancesL2 } from '../../../hooks/useGetAllBalancesL2';
import { useAccount } from 'wagmi';
import { ampli } from '../../../shared/analytics';

export function DepositLayer1() {
    const addressL1 = useCurrentAccount()?.address as string;
    const addressL2 = useAccount().address as `0x${string}`;
    const client = useIotaClient();
    const { mutateAsync: signAndExecuteTransaction, isPending: isTransactionLoading } =
        useSignAndExecuteTransaction();

    const { watch } = useFormContext<DepositFormData>();
    const { depositAmount, receivingAddress, coinType: selectedCoinType } = watch();

    const { refetch: refetchL2Balance } = useGetAllBalancesL2(addressL2);

    // Get all coins of the type
    const { data: selectedCoinObjects = [] } = useGetAllCoins(selectedCoinType, addressL1);

    const { data: coinMetadata } = useCoinMetadata(selectedCoinType);

    const amount = parseAmount(depositAmount, coinMetadata?.decimals ?? IOTA_DECIMALS) || BigInt(0);

    const { data: transactionData, isPending: isBuildingTransaction } =
        useBuildDepositTransactionL1({
            receivingAddress,
            amount,
            coins: selectedCoinObjects,
            coinType: selectedCoinType,
            refetchInterval: 2000,
        });
    const gasSummary = transactionData?.gasSummary;
    const formattedGasEstimation = gasSummary?.totalGas
        ? formatBalance(BigInt(gasSummary.totalGas), IOTA_DECIMALS, CoinFormat.Full)
        : undefined;

    const deposit = async () => {
        if (!transactionData?.transaction) {
            throw Error('Transaction is missing');
        }
        await signAndExecuteTransaction(
            {
                transaction: transactionData.transaction,
                options: {
                    showEffects: true,
                    showEvents: true,
                    showObjectChanges: true,
                },
            },
            {
                onSuccess: (tx) => {
                    toast('Deposit transaction submitted!');
                    client
                        .waitForTransaction({
                            digest: tx.digest,
                        })
                        .then(() => {
                            toast.success('Deposit transaction confirmed!');
                            refetchL2Balance();
                            ampli.sentFromL1ToL2({
                                amount: depositAmount,
                                coinType: selectedCoinType,
                            });
                        })
                        .catch((err) => {
                            if (import.meta.env.DEV) {
                                console.error(
                                    'Error while waiting for deposit transaction',
                                    err.message,
                                );
                            }
                        });
                },
                onError: (err) => {
                    if (err.message) {
                        if (err.message.startsWith(L1_USER_REJECTED_TX_ERROR_TEXT)) {
                            toast.error('Transaction canceled by user.');
                        } else {
                            toast.error(err.message);
                        }
                    } else {
                        toast.error('Unable to complete deposit transaction.');
                    }
                },
            },
        );
    };

    return (
        <DepositForm
            deposit={deposit}
            isGasEstimationLoading={isBuildingTransaction}
            isTransactionLoading={isTransactionLoading}
            gasEstimation={formattedGasEstimation}
            gasEstimationEVM={formatBalance(L2_FROM_L1_GAS_BUDGET, IOTA_DECIMALS, CoinFormat.Full)}
        />
    );
}
