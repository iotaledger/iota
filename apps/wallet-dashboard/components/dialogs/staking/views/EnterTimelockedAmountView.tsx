// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useMemo, useState } from 'react';
import {
    useFormatCoin,
    CoinFormat,
    useGetAllOwnedObjects,
    TIMELOCK_IOTA_TYPE,
    SIZE_LIMIT_EXCEEDED,
    useGetClockTimestamp,
    toast,
    parseAmount,
    useCoinMetadata,
    getGasBudgetErrorMessage,
} from '@iota/core';
import { IOTA_TYPE_ARG, NANOS_PER_IOTA } from '@iota/iota-sdk/utils';
import { useFormikContext } from 'formik';
import { useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { getAmountFromGroupedTimelockObjects, useNewStakeTimelockedTransaction } from '@/hooks';
import { prepareObjectsForTimelockedStakingTransaction } from '@/lib/utils';
import { EnterAmountDialogLayout } from './EnterAmountDialogLayout';
import { ampli } from '@/lib/utils/analytics';
import { InfoBox, InfoBoxStyle, InfoBoxType } from '@iota/apps-ui-kit';
import { Exclamation } from '@iota/apps-ui-icons';

interface FormValues {
    amount: string;
}

interface EnterTimelockedAmountViewProps {
    selectedValidator: string;
    maxStakableTimelockedAmount: bigint;
    senderAddress: string;
    onBack: () => void;
    handleClose: () => void;
    onSuccess: (digest: string) => void;
}

// number of iota for decrease by each attempt
const REDUCTION_STEP_SIZE = BigInt(1_000_000_000);

export function EnterTimelockedAmountView({
    selectedValidator,
    maxStakableTimelockedAmount,
    senderAddress,
    onBack,
    handleClose,
    onSuccess,
}: EnterTimelockedAmountViewProps): JSX.Element {
    const { mutateAsync: signAndExecuteTransaction } = useSignAndExecuteTransaction();
    const { values, resetForm } = useFormikContext<FormValues>();
    const [possibleAmount, setPossibleAmount] = useState<bigint | null>(null);
    const [isSearchingProtocolMaxAmount, setSearchingProtocolMaxAmount] = useState(false);
    const { data: metadata } = useCoinMetadata(IOTA_TYPE_ARG);
    const decimals = metadata?.decimals ?? 0;
    const amountWithoutDecimals = parseAmount(values.amount, decimals);

    const { data: clockTimestampMs } = useGetClockTimestamp();
    const { data: timelockedObjects } = useGetAllOwnedObjects(senderAddress, {
        StructType: TIMELOCK_IOTA_TYPE,
    });
    const groupedTimelockObjects = useMemo(() => {
        if (!timelockedObjects || possibleAmount === null) return [];
        return prepareObjectsForTimelockedStakingTransaction(
            timelockedObjects,
            possibleAmount,
            clockTimestampMs,
        );
    }, [timelockedObjects, clockTimestampMs, possibleAmount]);

    const {
        data: newStakeData,
        isLoading: isTransactionLoading,
        isError,
        error: stakeTransactionError,
    } = useNewStakeTimelockedTransaction(selectedValidator, senderAddress, groupedTimelockObjects);

    const stakedAmount = getAmountFromGroupedTimelockObjects(groupedTimelockObjects);

    const hasGroupedTimelockObjects = groupedTimelockObjects.length > 0;

    const [maxTokenFormatted, maxTokenFormattedSymbol] = useFormatCoin({
        balance: maxStakableTimelockedAmount,
        format: CoinFormat.FULL,
    });

    const [possibleAmountFormatted, possibleAmountSymbol] = useFormatCoin({
        balance: possibleAmount,
        format: CoinFormat.FULL,
    });

    const caption = `${maxTokenFormatted} ${maxTokenFormattedSymbol} Available`;
    const info = useMemo(() => {
        if (isSearchingProtocolMaxAmount) {
            let message = 'The current amount is not valid due to the large number of objects. ';

            message += isTransactionLoading
                ? 'Determining a valid amount...'
                : `Valid amount: ${possibleAmountFormatted} ${possibleAmountSymbol}`;

            return {
                title: 'Partial staking',
                message: message,
            };
        }
        if (!hasGroupedTimelockObjects && possibleAmountFormatted) {
            return {
                message:
                    'Combining timelocked objects to stake the entered amount is not possible. Please try a different amount.',
            };
        }

        return {
            message: '',
        };
    }, [
        hasGroupedTimelockObjects,
        isSearchingProtocolMaxAmount,
        isTransactionLoading,
        possibleAmountFormatted,
        possibleAmountSymbol,
    ]);

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

    useEffect(() => {
        if (!amountWithoutDecimals) {
            setPossibleAmount(null);
        } else {
            setPossibleAmount(amountWithoutDecimals);
        }
        setSearchingProtocolMaxAmount(false);
    }, [amountWithoutDecimals]);

    useEffect(() => {
        if (
            isError &&
            possibleAmount &&
            stakeTransactionError?.message.includes(SIZE_LIMIT_EXCEEDED)
        ) {
            setSearchingProtocolMaxAmount(true);
            setPossibleAmount(possibleAmount - REDUCTION_STEP_SIZE);
        }
    }, [isError, possibleAmount, stakeTransactionError]);

    const errorMessage = useMemo(() => {
        if (isError) {
            return getGasBudgetErrorMessage(stakeTransactionError);
        } else {
            return undefined;
        }
    }, [stakeTransactionError, isError]);

    return (
        <EnterAmountDialogLayout
            selectedValidator={selectedValidator}
            totalGas={newStakeData?.gasSummary?.totalGas}
            senderAddress={senderAddress}
            caption={caption}
            renderInfo={
                info.message ? (
                    <InfoBox
                        title={info.title}
                        type={InfoBoxType.Error}
                        supportingText={info.message}
                        style={InfoBoxStyle.Elevated}
                        icon={<Exclamation />}
                    />
                ) : undefined
            }
            isLoading={isTransactionLoading}
            isStakeDisabled={!hasGroupedTimelockObjects || isSearchingProtocolMaxAmount}
            onBack={onBack}
            handleClose={handleClose}
            handleStake={handleStake}
            errorMessage={errorMessage}
        />
    );
}
