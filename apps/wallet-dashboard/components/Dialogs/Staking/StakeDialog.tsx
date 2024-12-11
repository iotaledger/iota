// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useMemo, useState } from 'react';
import { EnterAmountView, EnterTimelockedAmountView, SelectValidatorView } from './views';
import {
    ExtendedDelegatedStake,
    parseAmount,
    useCoinMetadata,
    useGetValidatorsApy,
    useBalance,
    createValidationSchema,
    MIN_NUMBER_IOTA_TO_STAKE,
} from '@iota/core';
import { FormikProvider, useFormik } from 'formik';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Dialog } from '@iota/apps-ui-kit';
import { DetailsView } from './views';
import { TransactionDialogView } from '../TransactionDialog';
import { StakeDialogView } from './enums/view.enums';

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
    onUnstakeClick?: () => void;
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
    onUnstakeClick,
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
                false,
                minimumStake,
            ),
        [maxStakableTimelockedAmount, coinBalance, coinSymbol, coinDecimals, minimumStake],
    );

    const formik = useFormik({
        initialValues: INITIAL_VALUES,
        validationSchema: validationSchema,
        onSubmit: () => undefined,
        validateOnMount: true,
    });

    const amount = formik.values.amount || `${MIN_NUMBER_IOTA_TO_STAKE}`;
    const amountWithoutDecimals = parseAmount(amount, coinDecimals);

    const { data: rollingAverageApys } = useGetValidatorsApy();

    const validators = Object.keys(rollingAverageApys ?? {}) ?? [];

    function handleBack(): void {
        setView(StakeDialogView.SelectValidator);
    }

    function handleValidatorSelect(validator: string): void {
        setSelectedValidator?.(validator);
    }

    function setViewBasedOnStakingType() {
        setView(
            isTimelockedStaking
                ? StakeDialogView.EnterTimelockedAmount
                : StakeDialogView.EnterAmount,
        );
    }

    function selectValidatorHandleNext(): void {
        if (selectedValidator) {
            setViewBasedOnStakingType();
        }
    }

    function detailsHandleStake() {
        setSelectedValidator?.(stakedDetails?.validatorAddress ?? '');
        setViewBasedOnStakingType();
    }

    function handleTransactionSuccess(digest: string) {
        onSuccess?.(digest);
        setTxDigest(digest);
        setView(StakeDialogView.TransactionDetails);
    }

    return (
        <Dialog open={isOpen} onOpenChange={() => handleClose()}>
            <FormikProvider value={formik}>
                <>
                    {view === StakeDialogView.Details && stakedDetails && (
                        <DetailsView
                            handleStake={detailsHandleStake}
                            handleUnstake={onUnstakeClick}
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
                            amountWithoutDecimals={amountWithoutDecimals}
                            senderAddress={senderAddress}
                            onSuccess={handleTransactionSuccess}
                        />
                    )}
                    {view === StakeDialogView.EnterTimelockedAmount && (
                        <EnterTimelockedAmountView
                            selectedValidator={selectedValidator}
                            maxStakableTimelockedAmount={maxStakableTimelockedAmount ?? BigInt(0)}
                            handleClose={handleClose}
                            onBack={handleBack}
                            senderAddress={senderAddress}
                            onSuccess={handleTransactionSuccess}
                            amountWithoutDecimals={amountWithoutDecimals}
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
