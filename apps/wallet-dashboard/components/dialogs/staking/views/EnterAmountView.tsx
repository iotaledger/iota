// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin, useBalance, CoinFormat, parseAmount, useCoinMetadata } from '@iota/core';
import { IOTA_TYPE_ARG, NANOS_PER_IOTA } from '@iota/iota-sdk/utils';
import { useFormikContext } from 'formik';
import { useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { useNewStakeTransaction } from '@/hooks';
import { EnterAmountDialogLayout } from './EnterAmountDialogLayout';
import toast from 'react-hot-toast';
import { ampli } from '@/lib/utils/analytics';

export interface FormValues {
    amount: string;
}

interface EnterAmountViewProps {
    selectedValidator: string;
    onBack: () => void;
    showActiveStatus?: boolean;
    handleClose: () => void;
    amountWithoutDecimals: bigint;
    senderAddress: string;
    onSuccess: (digest: string) => void;
}

export function EnterAmountView({
    selectedValidator,
    onBack,
    handleClose,
    amountWithoutDecimals,
    senderAddress,
    onSuccess,
}: EnterAmountViewProps): JSX.Element {
    const { mutateAsync: signAndExecuteTransaction } = useSignAndExecuteTransaction();
    const { values, resetForm } = useFormikContext<FormValues>();

    const coinType = IOTA_TYPE_ARG;
    const { data: metadata } = useCoinMetadata(coinType);
    const decimals = metadata?.decimals ?? 0;

    const { data: iotaBalance } = useBalance(senderAddress);
    const coinBalance = BigInt(iotaBalance?.totalBalance || 0);

    const { data: newStakeData, isLoading: isTransactionLoading } = useNewStakeTransaction(
        selectedValidator,
        amountWithoutDecimals,
        senderAddress,
    );

    const gasBudgetBigInt = BigInt(newStakeData?.gasBudget ?? 0);
    const maxTokenBalance = coinBalance - gasBudgetBigInt;
    const [maxTokenFormatted, maxTokenFormattedSymbol] = useFormatCoin(
        maxTokenBalance,
        IOTA_TYPE_ARG,
        CoinFormat.FULL,
    );

    const caption = `${maxTokenFormatted} ${maxTokenFormattedSymbol} Available`;
    const infoMessage =
        'You have selected an amount that will leave you with insufficient funds to pay for gas fees for unstaking or any other transactions.';
    const hasEnoughRemaingBalance =
        maxTokenBalance > parseAmount(values.amount, decimals) + BigInt(2) * gasBudgetBigInt;

    function handleStake(): void {
        if (!newStakeData?.transaction) {
            toast.error('Stake transaction was not created');
            return;
        }
        signAndExecuteTransaction(
            {
                transaction: newStakeData?.transaction,
            },
            {
                onSuccess: (tx) => {
                    onSuccess(tx.digest);
                    toast.success('Stake transaction has been sent');
                    resetForm();
                    ampli.stakedIota({
                        stakedAmount: Number(BigInt(values.amount) / NANOS_PER_IOTA),
                    });
                },
                onError: () => {
                    toast.error('Stake transaction was not sent');
                },
            },
        );
    }

    return (
        <EnterAmountDialogLayout
            selectedValidator={selectedValidator}
            gasBudget={newStakeData?.gasBudget}
            senderAddress={senderAddress}
            caption={caption}
            showInfo={!hasEnoughRemaingBalance}
            infoMessage={infoMessage}
            isLoading={isTransactionLoading}
            onBack={onBack}
            handleClose={handleClose}
            handleStake={handleStake}
        />
    );
}
