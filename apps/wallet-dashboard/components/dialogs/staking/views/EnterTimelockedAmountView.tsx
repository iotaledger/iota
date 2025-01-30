// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo } from 'react';
import { useFormatCoin, CoinFormat, useGetAllOwnedObjects, TIMELOCK_IOTA_TYPE } from '@iota/core';
import { IOTA_TYPE_ARG, NANOS_PER_IOTA } from '@iota/iota-sdk/utils';
import { useFormikContext } from 'formik';
import { useSignAndExecuteTransaction } from '@iota/dapp-kit';
import {
    getAmountFromGroupedTimelockObjects,
    useGetCurrentEpochStartTimestamp,
    useNewStakeTimelockedTransaction,
} from '@/hooks';
import { prepareObjectsForTimelockedStakingTransaction } from '@/lib/utils';
import { EnterAmountDialogLayout } from './EnterAmountDialogLayout';
import toast from 'react-hot-toast';
import { ampli } from '@/lib/utils/analytics';

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
    const groupedTimelockObjects = useMemo(() => {
        if (!timelockedObjects || !currentEpochMs) return [];
        return prepareObjectsForTimelockedStakingTransaction(
            timelockedObjects,
            amountWithoutDecimals,
            currentEpochMs,
        );
    }, [timelockedObjects, currentEpochMs, amountWithoutDecimals]);

    const { data: newStakeData, isLoading: isTransactionLoading } =
        useNewStakeTimelockedTransaction(selectedValidator, senderAddress, groupedTimelockObjects);

    const stakedAmount = getAmountFromGroupedTimelockObjects(groupedTimelockObjects);

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
                    ampli.timelockStake({
                        stakedAmount: Number(stakedAmount / NANOS_PER_IOTA),
                        validatorAddress: senderAddress,
                    });
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
