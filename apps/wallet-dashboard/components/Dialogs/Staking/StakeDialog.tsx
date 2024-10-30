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
    GroupedTimelockObject,
    parseAmount,
    TIMELOCK_IOTA_TYPE,
    useCoinMetadata,
    useGetAllOwnedObjects,
    useGetValidatorsApy,
} from '@iota/core';
import { useCurrentAccount, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { NotificationType } from '@/stores/notificationStore';
import { prepareObjectsForTimelockedStakingTransaction } from '@/lib/utils';
import { Dialog, DialogBody, DialogContent, DialogPosition, Header } from '@iota/apps-ui-kit';

interface StakeDialogProps {
    isTimelockedStaking?: boolean;
    onSuccess?: (digest: string) => void;
    isOpen: boolean;
    setOpen: (bool: boolean) => void;
}

enum Step {
    SelectValidator,
    EnterAmount,
}

function StakeDialog({
    onSuccess,
    isTimelockedStaking,
    isOpen,
    setOpen,
}: StakeDialogProps): JSX.Element {
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

    let groupedTimelockObjects: GroupedTimelockObject[] = [];
    if (isTimelockedStaking && timelockedObjects && currentEpochMs) {
        groupedTimelockObjects = prepareObjectsForTimelockedStakingTransaction(
            timelockedObjects,
            amountWithoutDecimals,
            currentEpochMs,
        );
    }

    const { data: newStakeData } = useNewStakeTransaction(
        selectedValidator,
        amountWithoutDecimals,
        senderAddress,
        isTimelockedStaking,
        groupedTimelockObjects,
    );

    const { mutateAsync: signAndExecuteTransaction } = useSignAndExecuteTransaction();
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
        if (isTimelockedStaking && groupedTimelockObjects.length === 0) {
            addNotification('Invalid stake amount. Please try again.', NotificationType.Error);
            return;
        }
        if (!newStakeData?.transaction) {
            addNotification('Stake transaction was not created', NotificationType.Error);
            return;
        }
        signAndExecuteTransaction(
            {
                transaction: newStakeData?.transaction,
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
                addNotification('Stake transaction has been sent');
            })
            .catch(() => {
                addNotification('Stake transaction was not sent', NotificationType.Error);
            });
    }

    return (
        <Dialog open={isOpen} onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container" position={DialogPosition.Right}>
                <Header title="Receive" onClose={() => setOpen(false)} />
                <DialogBody>
                    {step === Step.SelectValidator && (
                        <SelectValidatorView
                            validators={validators}
                            onSelect={handleValidatorSelect}
                        />
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
                </DialogBody>
            </DialogContent>
        </Dialog>
    );
}

export default StakeDialog;
