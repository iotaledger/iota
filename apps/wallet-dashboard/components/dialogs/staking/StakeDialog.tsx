// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo, useState } from 'react';
import { EnterAmountView, EnterTimelockedAmountView, SelectValidatorView } from './views';
import {
    ExtendedDelegatedStake,
    parseAmount,
    useCoinMetadata,
    useBalance,
    createValidationSchema,
    MIN_NUMBER_IOTA_TO_STAKE,
} from '@iota/core';
import { FormikProvider, useFormik } from 'formik';
import { useCurrentAccount, useIotaClientQuery } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Dialog } from '@iota/apps-ui-kit';
import { DetailsView } from './views';
import { TransactionDialogView } from '../TransactionDialog';
import { StakeDialogView } from './enums/view.enums';
import { ampli } from '@/lib/utils/analytics';

const INITIAL_VALUES = {
    amount: '',
};

interface StakeDialogProps {
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
    const [txDigest, setTxDigest] = useState<string | null>(null);

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

    const { data: systemState } = useIotaClientQuery('getLatestIotaSystemState');
    const validatorAddresses = (systemState?.activeValidators ?? []).map(
        (validator) => validator.iotaAddress,
    );

    const amount = formik.values.amount;
    const amountWithoutDecimals = parseAmount(amount, coinDecimals);

    function handleBack(): void {
        setView(StakeDialogView.SelectValidator);
    }

    function handleValidatorSelect(validator: string): void {
        setSelectedValidator?.(validator);

        ampli.selectValidator({
            validatorAddress: validator,
        });
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
        if (stakedDetails) {
            setSelectedValidator?.(stakedDetails.validatorAddress);
            setViewBasedOnStakingType();
        }
    }

    function handleTransactionSuccess(digest: string) {
        onSuccess?.(digest);
        setTxDigest(digest);
        setView(StakeDialogView.TransactionDetails);
    }

    return (
        <Dialog open onOpenChange={() => handleClose()}>
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
                            validators={validatorAddresses}
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
