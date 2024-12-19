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
import {
    useGetCurrentEpochStartTimestamp,
    useNewStakeTimelockedTransaction,
    useNotifications,
} from '@/hooks';
import { NotificationType } from '@/stores/notificationStore';
import { prepareObjectsForTimelockedStakingTransaction } from '@/lib/utils';
import EnterAmountDialogLayout from './EnterAmountDialogLayout';

export interface FormValues {
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

function EnterTimelockedAmountView({
    selectedValidator,
    maxStakableTimelockedAmount,
    amountWithoutDecimals,
    senderAddress,
    onBack,
    handleClose,
    onSuccess,
}: EnterTimelockedAmountViewProps): JSX.Element {
    const { addNotification } = useNotifications();
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
                    resetForm();
                },
                onError: () => {
                    addNotification('Stake transaction was not sent', NotificationType.Error);
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

export default EnterTimelockedAmountView;
