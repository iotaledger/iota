// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useMemo } from 'react';
import { EnterAmountView, SelectValidatorView } from './views';
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
} from '@iota/core';
import { FormikProvider, useFormik } from 'formik';
import type { FormikHelpers } from 'formik';
import { useCurrentAccount, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { NotificationType } from '@/stores/notificationStore';
import { prepareObjectsForTimelockedStakingTransaction } from '@/lib/utils';
import { DetailsView, UnstakeView } from './views';
import { Dialog } from '@iota/apps-ui-kit';
import { FinishStakingView } from './views/FinishStaking';
import { FormValues } from './views/EnterAmountView';

export const MIN_NUMBER_IOTA_TO_STAKE = 1;

export enum StakeDialogView {
    Details = 'Details',
    SelectValidator = 'SelectValidator',
    EnterAmount = 'EnterAmount',
    Unstake = 'Unstake',
    TransactionDetails = 'TransactionDetails',
}

const INITIAL_VALUES = {
    amount: '',
};

interface StakeDialogProps {
    isTimelockedStaking?: boolean;
    isOpen: boolean;
    handleClose: () => void;
    view: StakeDialogView;
    setView?: (view: StakeDialogView) => void;
    stakedDetails?: ExtendedDelegatedStake | null;

    selectedValidator?: string;
    setSelectedValidator?: (validator: string) => void;
}

export function StakeDialog({
    isTimelockedStaking,
    isOpen,
    handleClose,
    view,
    setView,
    stakedDetails,
    selectedValidator = '',
    setSelectedValidator,
}: StakeDialogProps): JSX.Element {
    const account = useCurrentAccount();
    const senderAddress = account?.address ?? '';
    const { data: iotaBalance } = useBalance(senderAddress!);
    const coinBalance = BigInt(iotaBalance?.totalBalance || 0);

    const { data: metadata } = useCoinMetadata(IOTA_TYPE_ARG);
    const coinDecimals = metadata?.decimals ?? 0;
    const coinSymbol = metadata?.symbol ?? '';
    const minimumStake = parseAmount(MIN_NUMBER_IOTA_TO_STAKE.toString(), coinDecimals);

    const validationSchema = useMemo(
        () =>
            createValidationSchema(
                coinBalance,
                coinSymbol,
                coinDecimals,
                view === StakeDialogView.Unstake,
                minimumStake,
            ),
        [coinBalance, coinSymbol, coinDecimals, view, minimumStake],
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

    const { mutateAsync: signAndExecuteTransaction } = useSignAndExecuteTransaction();
    const { addNotification } = useNotifications();
    const { data: rollingAverageApys } = useGetValidatorsApy();

    const validators = Object.keys(rollingAverageApys ?? {}) ?? [];

    const validatorApy =
        rollingAverageApys && selectedValidator ? rollingAverageApys[selectedValidator] : null;

    function handleBack(): void {
        setView?.(StakeDialogView.SelectValidator);
    }

    function handleValidatorSelect(validator: string): void {
        setSelectedValidator?.(validator);
    }

    function selectValidatorHandleNext(): void {
        if (selectedValidator) {
            setView?.(StakeDialogView.EnterAmount);
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
                onSuccess: () => {
                    setView?.(StakeDialogView.TransactionDetails);
                    addNotification('Stake transaction has been sent');
                },
            },
        ).catch(() => {
            addNotification('Stake transaction was not sent', NotificationType.Error);
        });
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
                    {view === StakeDialogView.EnterAmount && validatorApy && (
                        <EnterAmountView
                            selectedValidator={selectedValidator}
                            handleClose={handleClose}
                            onBack={handleBack}
                            onStake={handleStake}
                            gasBudget={newStakeData?.gasBudget}
                            validatorApy={validatorApy}
                            isTransactionLoading={isTransactionLoading}
                        />
                    )}
                    {view === StakeDialogView.Unstake && stakedDetails && (
                        <UnstakeView
                            extendedStake={stakedDetails}
                            onUnstake={handleClose}
                            showActiveStatus
                        />
                    )}
                    {view === StakeDialogView.TransactionDetails && validatorApy !== null && (
                        <FinishStakingView
                            validatorAddress={selectedValidator}
                            gasBudget={newStakeData?.gasBudget}
                            onConfirm={handleClose}
                            onClose={handleClose}
                            amount={amount}
                            symbol={metadata?.symbol}
                            validatorApy={validatorApy}
                        />
                    )}
                </>
            </FormikProvider>
        </Dialog>
    );
}
