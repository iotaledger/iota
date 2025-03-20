// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    useFormatCoin,
    useBalance,
    CoinFormat,
    useCoinMetadata,
    safeParseAmount,
    toast,
    useNewStakeTransaction,
    parseAmount,
    formatBalance,
} from '@iota/core';
import { IOTA_DECIMALS, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useFormikContext } from 'formik';
import { useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { EnterAmountDialogLayout } from './EnterAmountDialogLayout';
import { ampli } from '@/lib/utils/analytics';
import { useEffect } from 'react';
import { ButtonPill } from '@iota/apps-ui-kit';

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
    const { values, resetForm, setFieldValue } = useFormikContext<FormValues>();

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

    const gasSummary = newStakeData?.gasSummary;

    const { data: maxAmountTransactionData } = useNewStakeTransaction(
        selectedValidator,
        coinBalance,
        senderAddress,
    );
    const maxAmountTxGasBudget = BigInt(maxAmountTransactionData?.gasSummary?.budget ?? 0n);
    const maxAmountTxTotalGas = BigInt(maxAmountTransactionData?.gasSummary?.totalGas ?? 0n);
    const maxAmountTxStorageRebate = BigInt(
        maxAmountTransactionData?.gasSummary?.storageRebate ?? 0n,
    );

    useEffect(() => {
        setFieldValue('gasBudget', maxAmountTxGasBudget);
    }, [maxAmountTxGasBudget, setFieldValue]);

    const maxTokenBalance = coinBalance - maxAmountTxGasBudget;
    const [maxTokenFormatted, maxTokenFormattedSymbol] = useFormatCoin({
        balance: maxTokenBalance,
        format: CoinFormat.FULL,
    });

    const caption = maxAmountTxGasBudget
        ? `${maxTokenFormatted} ${maxTokenFormattedSymbol} Available`
        : '--';
    const infoMessage =
        'You have selected an amount that will leave you with insufficient funds to pay for gas fees for unstaking or any other transactions.';

    const hasAmount = values.amount.length > 0;
    const amount = safeParseAmount(coinType === IOTA_TYPE_ARG ? values.amount : '0', decimals);
    const gasUnstakeBuffer = (maxAmountTxTotalGas + maxAmountTxStorageRebate) * BigInt(2); // 2x gas budget need for unstaking

    const amountWithGas = amount !== null ? amount + gasUnstakeBuffer : 0n;
    const canPay = amount !== null ? maxTokenBalance >= amount + gasUnstakeBuffer : false;
    const hasEnoughRemainingBalance = !(hasAmount && !canPay);
    const isMaxAmountSet = maxTokenBalance === amountWithGas;

    function onActionClick() {
        const maxSafeAmount = maxTokenBalance - gasUnstakeBuffer;
        const maxSafeAmountFormatted = formatBalance(maxSafeAmount, decimals, CoinFormat.FULL);

        setFieldValue('amount', maxSafeAmountFormatted, true);
    }

    const renderAction = () => <ButtonPill onClick={onActionClick}>Max</ButtonPill>;

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
                    ampli.stakedIota({
                        stakedAmount: Number(parseAmount(values.amount, IOTA_DECIMALS)),
                    });
                    resetForm();
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
            totalGas={gasSummary?.totalGas}
            senderAddress={senderAddress}
            caption={caption}
            showInfo={!hasEnoughRemainingBalance}
            infoMessage={infoMessage}
            isLoading={isTransactionLoading}
            onBack={onBack}
            handleClose={handleClose}
            handleStake={handleStake}
            renderInputAction={renderAction}
            isMaxAmountSet={isMaxAmountSet}
        />
    );
}
