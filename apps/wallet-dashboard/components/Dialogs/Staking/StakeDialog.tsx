// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useMemo, useState } from 'react';
import { EnterAmountView, EnterTimelockedAmountView, SelectValidatorView } from './views';
import {
    useNotifications,
    useNewStakeTransaction,
    useGetCurrentEpochStartTimestamp,
} from '@/hooks';
import {
    ExtendedDelegatedStake,
    GroupedTimelockObject,
    parseAmount,
    TIMELOCK_IOTA_TYPE,
    useCoinMetadata,
    useGetAllOwnedObjects,
    useGetValidatorsApy,
    useBalance,
    createValidationSchema,
    MIN_NUMBER_IOTA_TO_STAKE,
} from '@iota/core';
import { FormikProvider, useFormik } from 'formik';
import type { FormikHelpers } from 'formik';
import { useCurrentAccount, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { NotificationType } from '@/stores/notificationStore';
import { prepareObjectsForTimelockedStakingTransaction } from '@/lib/utils';
import { Dialog } from '@iota/apps-ui-kit';
import { DetailsView, UnstakeView } from './views';
import { FormValues } from './views/EnterAmountView';
import { TransactionDialogView } from '../TransactionDialog';
import { StakeDialogView } from './enums/view.enums';
import { Transaction } from '@iota/iota-sdk/transactions';

const INITIAL_VALUES = {
    amount: '',
};

interface StakeDialogProps {
    isOpen: boolean;
    handleClose: () => void;
    view: StakeDialogView | undefined;
    setView: (view: StakeDialogView) => void;
    stakedDetails?: ExtendedDelegatedStake | null;
    maxStakableTimelockedAmount?: bigint;
    isTimelockedStaking?: boolean;
    onSuccess?: (digest: string) => void;
    selectedValidator?: string;
    setSelectedValidator?: (validator: string) => void;
}

export function StakeDialog({
    onSuccess,
    isTimelockedStaking,
    isOpen,
    handleClose,
    view,
    setView,
    stakedDetails,
    maxStakableTimelockedAmount,
    selectedValidator = '',
    setSelectedValidator,
}: StakeDialogProps): JSX.Element {
    const account = useCurrentAccount();
    const senderAddress = account?.address ?? '';
    const { data: iotaBalance } = useBalance(senderAddress!);
    const coinBalance = BigInt(iotaBalance?.totalBalance || 0);
    const [txDigest, setTxDigest] = useState<string>('');

    const { data: metadata } = useCoinMetadata(IOTA_TYPE_ARG);
    const coinDecimals = metadata?.decimals ?? 0;
    const coinSymbol = metadata?.symbol ?? '';
    const minimumStake = parseAmount(MIN_NUMBER_IOTA_TO_STAKE.toString(), coinDecimals);

    const validationSchema = useMemo(
        () =>
            createValidationSchema(
                maxStakableTimelockedAmount ?? coinBalance,
                coinSymbol,
                coinDecimals,
                view === StakeDialogView.Unstake,
                minimumStake,
            ),
        [maxStakableTimelockedAmount, coinBalance, coinSymbol, coinDecimals, view, minimumStake],
    );

    const formik = useFormik({
        initialValues: INITIAL_VALUES,
        validationSchema: validationSchema,
        onSubmit: onSubmit,
        validateOnMount: true,
    });

    const amount = formik.values.amount || `${MIN_NUMBER_IOTA_TO_STAKE}`;
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

    const { data: newStakeData, isLoading: isTransactionLoading } = useNewStakeTransaction(
        selectedValidator,
        amountWithoutDecimals,
        senderAddress,
        isTimelockedStaking,
        groupedTimelockObjects,
    );

    const { mutateAsync: signAndExecuteTransaction, isPending } = useSignAndExecuteTransaction();
    const { addNotification } = useNotifications();
    const { data: rollingAverageApys } = useGetValidatorsApy();

    const validators = Object.keys(rollingAverageApys ?? {}) ?? [];

    function handleBack(): void {
        setView?.(StakeDialogView.SelectValidator);
    }

    function handleValidatorSelect(validator: string): void {
        setSelectedValidator?.(validator);
    }

    function selectValidatorHandleNext(): void {
        if (selectedValidator) {
            setView?.(
                isTimelockedStaking
                    ? StakeDialogView.EnterTimelockedAmount
                    : StakeDialogView.EnterAmount,
            );
        }
    }

    function detailsHandleUnstake() {
        setView?.(StakeDialogView.Unstake);
    }

    function detailsHandleStake() {
        setView?.(StakeDialogView.SelectValidator);
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
                    onSuccess?.(tx.digest);
                    addNotification('Stake transaction has been sent');
                    setTxDigest(tx.digest);
                    setView?.(StakeDialogView.TransactionDetails);
                },
                onError: () => {
                    addNotification('Stake transaction was not sent', NotificationType.Error);
                },
            },
        );
    }

    function handleUnstake(transaction: Transaction): void {
        if (!transaction) {
            addNotification('Unstake transaction was not created', NotificationType.Error);
            return;
        }
        signAndExecuteTransaction(
            {
                transaction: transaction,
            },
            {
                onSuccess: (tx) => {
                    onSuccess?.(tx.digest);
                    addNotification('Unstake transaction has been sent');
                    setTxDigest(tx.digest);
                    setView?.(StakeDialogView.TransactionDetails);
                },
                onError: () => {
                    addNotification('Unstake transaction was not sent', NotificationType.Error);
                },
            },
        );
    }

    function onSubmit(_: FormValues, { resetForm }: FormikHelpers<FormValues>) {
        handleStake();
        resetForm();
    }

    return (
        <Dialog open={isOpen} onOpenChange={() => handleClose()}>
            <FormikProvider value={formik}>
                <>
                    {view === StakeDialogView.Details && stakedDetails && (
                        <DetailsView
                            handleStake={detailsHandleStake}
                            handleUnstake={detailsHandleUnstake}
                            stakedDetails={stakedDetails}
                            handleClose={handleClose}
                        />
                    )}
                    {view === StakeDialogView.SelectValidator && (
                        <SelectValidatorView
                            selectedValidator={selectedValidator}
                            handleClose={handleClose}
                            validators={validators}
                            onSelect={handleValidatorSelect}
                            onNext={selectValidatorHandleNext}
                        />
                    )}
                    {view === StakeDialogView.EnterAmount && (
                        <EnterAmountView
                            selectedValidator={selectedValidator}
                            handleClose={handleClose}
                            onBack={handleBack}
                            onStake={handleStake}
                            gasBudget={newStakeData?.gasBudget}
                            isTransactionLoading={isTransactionLoading}
                        />
                    )}
                    {view === StakeDialogView.EnterTimelockedAmount && (
                        <EnterTimelockedAmountView
                            selectedValidator={selectedValidator}
                            maxStakableTimelockedAmount={maxStakableTimelockedAmount ?? BigInt(0)}
                            hasGroupedTimelockObjects={groupedTimelockObjects.length > 0}
                            handleClose={handleClose}
                            onBack={handleBack}
                            onStake={handleStake}
                            gasBudget={newStakeData?.gasBudget}
                            isTransactionLoading={isTransactionLoading}
                        />
                    )}
                    {view === StakeDialogView.Unstake && stakedDetails && (
                        <UnstakeView
                            extendedStake={stakedDetails}
                            handleClose={handleClose}
                            onUnstake={handleUnstake}
                            isPending={isPending}
                        />
                    )}
                    {view === StakeDialogView.TransactionDetails && (
                        <TransactionDialogView txDigest={txDigest} onClose={handleClose} />
                    )}
                </>
            </FormikProvider>
        </Dialog>
    );
}
