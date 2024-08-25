// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { EnterAmountView, SelectValidatorView } from './views';
import { useNotifications, useNewStakeTransaction } from '@/hooks';
import { parseAmount, useCoinMetadata, useGetValidatorsApy } from '@iota/core';
import { useCurrentAccount, useSignAndExecuteTransactionBlock } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { NotificationType } from '@/stores/notificationStore';

interface NewStakePopupProps {
    onClose: () => void;
    isTimelockedStaking?: boolean;
    onSuccess?: (digest: string) => void;
}

enum Step {
    SelectValidator,
    EnterAmount,
}

function NewStakePopup({
    onClose,
    onSuccess,
    isTimelockedStaking,
}: NewStakePopupProps): JSX.Element {
    const [step, setStep] = useState<Step>(Step.SelectValidator);
    const [selectedValidator, setSelectedValidator] = useState<string>('');
    const [amount, setAmount] = useState<string>('');
    const account = useCurrentAccount();

    const { data: metadata } = useCoinMetadata(IOTA_TYPE_ARG);
    const coinDecimals = metadata?.decimals ?? 0;
    const amountWithoutDecimals = parseAmount(amount, coinDecimals);

    const { data: newStakeData } = useNewStakeTransaction(
        selectedValidator,
        amountWithoutDecimals,
        account?.address ?? '',
        isTimelockedStaking,
    );

    const { mutateAsync: signAndExecuteTransactionBlock } = useSignAndExecuteTransactionBlock();
    const { addNotification } = useNotifications();
    const { data: rollingAverageApys } = useGetValidatorsApy();

    const validators = Object.keys(rollingAverageApys ?? {}) ?? [];

    function handleNext(): void {
        setStep(Step.EnterAmount);
    }

    function handleBack(): void {
        setStep(Step.SelectValidator);
    }

    function handleValidatorSelect(validator: string): void {
        setSelectedValidator(validator);
        handleNext();
    }

    function handleStake(): void {
        if (!newStakeData?.transaction) {
            addNotification('Stake transaction was not created', NotificationType.Error);
            return;
        }
        signAndExecuteTransactionBlock(
            {
                transactionBlock: newStakeData?.transaction,
            },
            {
                onSuccess: (tx) => {
                    if (onSuccess) {
                        onSuccess(tx.digest);
                    }
                },
            },
        )
            .then(() => {
                onClose();
                addNotification('Stake transaction has been sent');
            })
            .catch(() => {
                addNotification('Stake transaction was not sent', NotificationType.Error);
            });
    }

    return (
        <div className="flex min-w-[300px] flex-col gap-2">
            {step === Step.SelectValidator && (
                <SelectValidatorView validators={validators} onSelect={handleValidatorSelect} />
            )}
            {step === Step.EnterAmount && (
                <EnterAmountView
                    selectedValidator={selectedValidator}
                    amount={amount}
                    onChange={(e) => setAmount(e.target.value)}
                    onBack={handleBack}
                    onStake={handleStake}
                    isStakeDisabled={!amount}
                />
            )}
        </div>
    );
}

export default NewStakePopup;
