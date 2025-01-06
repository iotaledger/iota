// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useEffect, useState } from 'react';
import {
    useFormatCoin,
    CoinFormat,
    GroupedTimelockObject,
    useGetAllOwnedObjects,
    TIMELOCK_IOTA_TYPE,
} from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useFormikContext } from 'formik';
import { useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { useGetCurrentEpochStartTimestamp, useNewStakeTimelockedTransaction } from '@/hooks';
import { prepareObjectsForTimelockedStakingTransaction } from '@/lib/utils';
import { EnterAmountDialogLayout } from './EnterAmountDialogLayout';
import toast from 'react-hot-toast';

interface FormValues {
    amount: string;
}

interface EnterTimelockedAmountViewProps {
    selectedValidator: string;
    maxStakableTimelockedAmount: bigint;
    amountWithoutDecimals: bigint;
    senderAddress: string;
    onBack: () => void;
    handleClose: () => void;
    onSuccess: (digest: string) => void;
}

export function EnterTimelockedAmountView({
    selectedValidator,
    maxStakableTimelockedAmount,
    amountWithoutDecimals,
    senderAddress,
    onBack,
    handleClose,
    onSuccess,
}: EnterTimelockedAmountViewProps): JSX.Element {
    const { mutateAsync: signAndExecuteTransaction } = useSignAndExecuteTransaction();
    const { resetForm } = useFormikContext<FormValues>();

    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { data: timelockedObjects } = useGetAllOwnedObjects(senderAddress, {
        StructType: TIMELOCK_IOTA_TYPE,
    });
    const [groupedTimelockObjects, setGroupedTimelockObjects] = useState<GroupedTimelockObject[]>(
        [],
    );

    const { data: newStakeData, isLoading: isTransactionLoading } =
        useNewStakeTimelockedTransaction(selectedValidator, senderAddress, groupedTimelockObjects);

    useEffect(() => {
        if (timelockedObjects && currentEpochMs) {
            const groupedTimelockObjects = prepareObjectsForTimelockedStakingTransaction(
                timelockedObjects,
                amountWithoutDecimals,
                currentEpochMs,
            );
            setGroupedTimelockObjects(groupedTimelockObjects);
        }
    }, [timelockedObjects, currentEpochMs, amountWithoutDecimals]);

    const hasGroupedTimelockObjects = groupedTimelockObjects.length > 0;

    const [maxTokenFormatted, maxTokenFormattedSymbol] = useFormatCoin(
        maxStakableTimelockedAmount,
        IOTA_TYPE_ARG,
        CoinFormat.FULL,
    );

    const caption = `${maxTokenFormatted} ${maxTokenFormattedSymbol} Available`;
    const infoMessage =
        'It is not possible to combine timelocked objects to stake the entered amount. Please try a different amount.';

    function handleStake(): void {
        if (groupedTimelockObjects.length === 0) {
            toast.error('Invalid stake amount. Please try again.');
            return;
        }
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
                    onSuccess?.(tx.digest);
                    toast.success('Stake transaction has been sent');
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
            gasBudget={newStakeData?.gasBudget}
            senderAddress={senderAddress}
            caption={caption}
            showInfo={!hasGroupedTimelockObjects}
            infoMessage={infoMessage}
            isLoading={isTransactionLoading}
            isStakeDisabled={!hasGroupedTimelockObjects}
            onBack={onBack}
            handleClose={handleClose}
            handleStake={handleStake}
        />
    );
}
