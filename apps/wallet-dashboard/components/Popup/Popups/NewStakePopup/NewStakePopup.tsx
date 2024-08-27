// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { EnterAmountView, SelectValidatorView } from './views';
import {
    useNotifications,
    useNewStakeTransaction,
    useGetCurrentEpochStartTimestamp,
} from '@/hooks';
import {
    ExtendedTimelockObject,
    parseAmount,
    TIMELOCK_IOTA_TYPE,
    useCoinMetadata,
    useGetAllOwnedObjects,
    useGetValidatorsApy,
} from '@iota/core';
import { useCurrentAccount, useSignAndExecuteTransactionBlock } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { NotificationType } from '@/stores/notificationStore';
import { prepareObjectsForTimelockedStakingTransaction } from '@/lib/utils';

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
    const senderAddress = account?.address ?? '';

    const { data: metadata } = useCoinMetadata(IOTA_TYPE_ARG);
    const coinDecimals = metadata?.decimals ?? 0;
    const amountWithoutDecimals = parseAmount(amount, coinDecimals);
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { data: timelockedObjects } = useGetAllOwnedObjects(senderAddress, {
        StructType: TIMELOCK_IOTA_TYPE,
    });

    let extendedTimelockObjects: ExtendedTimelockObject[] = [];
    if (isTimelockedStaking && timelockedObjects && currentEpochMs) {
        extendedTimelockObjects = prepareObjectsForTimelockedStakingTransaction(
            timelockedObjects,
            amountWithoutDecimals,
            currentEpochMs,
        );
    }

    const { data: newStakeData } = useNewStakeTransaction(
        selectedValidator,
        amountWithoutDecimals,
        senderAddress,
        extendedTimelockObjects,
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
