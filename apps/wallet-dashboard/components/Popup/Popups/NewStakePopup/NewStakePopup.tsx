// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { EnterAmountView, SelectValidatorView } from './views';
import { useNotifications, useNewStakeTransaction } from '@/hooks';
import { NotificationType } from '@/stores/notificationStore';
import { useGetValidatorsApy } from '@iota/core';
import { useCurrentAccount, useSignAndExecuteTransactionBlock } from '@iota/dapp-kit';

interface NewStakePopupProps {
    onClose: () => void;
}

enum Step {
    SelectValidator,
    EnterAmount,
}

function NewStakePopup({ onClose }: NewStakePopupProps): JSX.Element {
    const [step, setStep] = useState<Step>(Step.SelectValidator);
    const [selectedValidator, setSelectedValidator] = useState<string>('');
    const [amount, setAmount] = useState<string>('');
    const account = useCurrentAccount();
    const { transaction } = useNewStakeTransaction(
        amount.toString(),
        selectedValidator,
        account?.address ?? '',
    );
    const { addNotification } = useNotifications();
    const { data: rollingAverageApys } = useGetValidatorsApy();
    const { mutateAsync: signAndExecuteTransactionBlock } = useSignAndExecuteTransactionBlock();

    const validators = Object.keys(rollingAverageApys ?? {}) ?? [];

    const handleNext = () => {
        setStep(Step.EnterAmount);
    };

    const handleBack = () => {
        setStep(Step.SelectValidator);
    };

    const handleValidatorSelect = (validator: string) => {
        setSelectedValidator(validator);
        handleNext();
    };

    const handleStake = async (): Promise<void> => {
        if (selectedValidator && amount) {
            if (transaction) {
                await signAndExecuteTransactionBlock({
                    transactionBlock: transaction,
                });
                onClose();
            } else {
                addNotification('Error creating transaction', NotificationType.Error);
            }
        }
    };

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
                    isStakeDisabled={!amount || !selectedValidator}
                />
            )}
        </div>
    );
}

export default NewStakePopup;
