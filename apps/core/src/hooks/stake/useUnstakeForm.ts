// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo, useState } from 'react';
import { useFormik } from 'formik';
import { CoinFormat, IOTA_DECIMALS } from '@iota/iota-sdk/utils';
import {
    MIN_STAKING_THRESHOLD,
    GAS_SYMBOL,
    NOT_ENOUGH_BALANCE_ID,
    GAS_BALANCE_TOO_LOW_ID,
} from '../../constants';
import {
    createUnstakeValidationSchema,
    parseUnstakeAmountNanos,
    calculateUnstakeBreakdown,
} from '../../utils';
import { useFormatCoin } from '../useFormatCoin';
import { useNewUnstakeTransaction } from './useNewUnstakeTransaction';
import { useNewPartialUnstakeTransaction } from './useNewPartialUnstakeTransaction';

const INITIAL_VALUES = {
    amount: '',
};

type FormValues = typeof INITIAL_VALUES;

interface UseUnstakeFormParams {
    activeAddress: string;
    stakedIotaId: string;
    principalAmount: bigint;
    rewardAmount: bigint;
}

export function useUnstakeForm({
    activeAddress,
    stakedIotaId,
    principalAmount,
    rewardAmount,
}: UseUnstakeFormParams) {
    const [isPartialUnstake, setIsPartialUnstake] = useState(false);

    // Validation schema for partial unstake amount
    const validationSchema = useMemo(
        () =>
            createUnstakeValidationSchema(
                principalAmount,
                GAS_SYMBOL,
                IOTA_DECIMALS,
                MIN_STAKING_THRESHOLD,
            ),
        [principalAmount],
    );

    const formik = useFormik<FormValues>({
        initialValues: INITIAL_VALUES,
        validationSchema,
        onSubmit: () => undefined,
        validateOnMount: false,
    });
    const { values, errors, resetForm } = formik;

    // Parse the unstake amount in nanos
    const unstakeAmountNanos = parseUnstakeAmountNanos(values.amount);

    // Calculate proportional rewards and remaining balances
    const breakdown = calculateUnstakeBreakdown({
        principalAmount,
        rewardAmount,
        unstakeAmountNanos,
        isPartialUnstake,
    });

    const {
        unstakeAmount,
        proportionalRewards,
        totalUnstakeAmount,
        remainingStake,
        remainingRewards,
        remainingTotalStaked,
    } = breakdown;

    // Formatted display values
    const [unstakeAmountFormatted] = useFormatCoin({ balance: unstakeAmount });
    const [rewardsFormatted, rewardSymbol] = useFormatCoin({ balance: proportionalRewards });
    const [totalUnstakeAmountFormatted] = useFormatCoin({ balance: totalUnstakeAmount });

    // Calculate remaining stake after unstake
    const [remainingStakeFormatted] = useFormatCoin({ balance: remainingStake });
    // Calculate remaining rewards after unstake
    const [remainingRewardsFormatted, remainingRewardsSymbol] = useFormatCoin({
        balance: remainingRewards,
    });
    // Calculate remaining total staked IOTA
    const [remainingTotalStakedFormatted] = useFormatCoin({ balance: remainingTotalStaked });

    // Plain formatted values (for analytics)
    const [unstakeAmountFormattedPlain] = useFormatCoin({
        balance: unstakeAmount,
        format: CoinFormat.Full,
        useGroupSeparator: false,
    });
    const [rewardsFormattedPlain] = useFormatCoin({
        balance: proportionalRewards,
        format: CoinFormat.Full,
        useGroupSeparator: false,
    });

    // Transaction queries
    const {
        data: unstakeData,
        isLoading: isFullUnstakeLoading,
        isError: isFullUnstakeError,
        error: fullUnstakeError,
    } = useNewUnstakeTransaction(activeAddress, stakedIotaId);

    const {
        data: partialUnstakeData,
        isLoading: isPartialUnstakeLoading,
        isError: isPartialUnstakeError,
        error: partialUnstakeError,
    } = useNewPartialUnstakeTransaction(activeAddress, stakedIotaId, unstakeAmountNanos);

    // Use partial unstake data if enabled, otherwise use full unstake
    const activeUnstakeData =
        isPartialUnstake && unstakeAmountNanos > 0n ? partialUnstakeData : unstakeData;
    const activeError = isPartialUnstake ? partialUnstakeError : fullUnstakeError;
    const activeIsError = isPartialUnstake ? isPartialUnstakeError : isFullUnstakeError;
    const activeIsLoading = isPartialUnstake ? isPartialUnstakeLoading : isFullUnstakeLoading;

    const transaction = activeUnstakeData?.transaction;

    const [gasFormatted, gasSymbol] = useFormatCoin({
        balance: activeUnstakeData?.gasSummary?.totalGas,
        format: CoinFormat.Full,
    });

    // Validation state
    const isInvalidPartialAmount =
        isPartialUnstake && (unstakeAmountNanos <= 0n || !!errors.amount);

    const isNotEnoughGas =
        activeError &&
        (activeError.message.includes(NOT_ENOUGH_BALANCE_ID) ||
            activeError.message.includes(GAS_BALANCE_TOO_LOW_ID));

    // Mode toggle handlers
    function switchToFullUnstake() {
        setIsPartialUnstake(false);
        resetForm();
    }

    function switchToPartialUnstake() {
        setIsPartialUnstake(true);
        resetForm();
    }

    return {
        // Formik
        formik,
        values,

        // Mode
        isPartialUnstake,
        switchToFullUnstake,
        switchToPartialUnstake,

        // Breakdown (raw)
        ...breakdown,

        // Formatted display values
        unstakeAmountFormatted,
        rewardsFormatted,
        rewardSymbol,
        totalUnstakeAmountFormatted,
        remainingStakeFormatted,
        remainingRewardsFormatted,
        remainingRewardsSymbol,
        remainingTotalStakedFormatted,

        // Plain formatted (for analytics)
        unstakeAmountFormattedPlain,
        rewardsFormattedPlain,

        // Transaction state
        activeUnstakeData,
        transaction,
        activeError,
        activeIsError,
        activeIsLoading,

        // Gas
        gasFormatted,
        gasSymbol,

        // Validation
        isInvalidPartialAmount,
        isNotEnoughGas,
    };
}
