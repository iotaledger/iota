// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    formatDelegatedStake,
    formatDelegatedTimelockedStake,
    mapTimelockObjects,
    TIMELOCK_IOTA_TYPE,
    useBalance,
    useFormatCoin,
    useGetAllOwnedObjects,
    useGetDelegatedStake,
    useGetTimelockedStakedObjects,
    useTotalDelegatedStake,
} from '@iota/core';
import { KeyValueInfo, Skeleton } from '@iota/apps-ui-kit';

interface BalanceBreakdownElement {
    keyText: string;
    value: string;
    supportingLabel: string;
    isLoading?: boolean;
    isError?: boolean;
}

export function AddressBalanceBreakdown({ address }: { address: string }): React.JSX.Element {
    const {
        data: balance,
        isLoading: isLoadingBalance,
        isError: isBalanceErrored,
    } = useBalance(address);

    const [totalLiquidBalance, symbol] = useFormatCoin({
        balance: balance?.totalBalance,
    });

    const {
        data: delegatedStake,
        isLoading: isLoadingDelegatedStakes,
        isError: isDelegatedStakeErrored,
    } = useGetDelegatedStake({
        address,
    });
    const delegatedStakes = delegatedStake ? formatDelegatedStake(delegatedStake) : [];
    const totalDelegatedStake = useTotalDelegatedStake(delegatedStakes);
    const [formattedDelegatedStake] = useFormatCoin({
        balance: totalDelegatedStake,
    });

    const {
        data: timelockedStakedObjects,
        isLoading: isLoadingTimelockedStakeObjects,
        isError: isTimelockedStakedObjectsErrored,
    } = useGetTimelockedStakedObjects(address);

    const extendedDelegatedTimelockedStakes = formatDelegatedTimelockedStake(
        timelockedStakedObjects || [],
    );

    const totalTimelockedStaked = extendedDelegatedTimelockedStakes?.reduce(
        (acc, extendedStake) => acc + BigInt(extendedStake.principal),
        0n,
    );

    const [formattedTimelockedStake] = useFormatCoin({
        balance: totalTimelockedStaked,
    });

    const {
        data: timelockedObjects,
        isLoading: isTimelockedObjectsLoading,
        isError: isTimelockedObjectsError,
    } = useGetAllOwnedObjects(address, {
        StructType: TIMELOCK_IOTA_TYPE,
    });

    const mappedTimelockedObjects = mapTimelockObjects(timelockedObjects || []);

    const totalTimelockedTokens = mappedTimelockedObjects.reduce(
        (acc, obj) => acc + BigInt(obj.locked.value),
        BigInt(0),
    );

    const [formattedTimelockedTokens] = useFormatCoin({
        balance: totalTimelockedTokens,
    });

    const totalBalanceBreakdown =
        BigInt(balance?.totalBalance || 0) +
        BigInt(totalDelegatedStake || 0) +
        BigInt(totalTimelockedStaked || 0) +
        BigInt(totalTimelockedTokens || 0);

    const [formattedTotalBalance] = useFormatCoin({
        balance: totalBalanceBreakdown,
    });

    const isLoadingTotalBalance =
        isLoadingBalance ||
        isLoadingDelegatedStakes ||
        isLoadingTimelockedStakeObjects ||
        isTimelockedObjectsLoading;

    const isTotalBalanceErrored =
        isBalanceErrored ||
        isDelegatedStakeErrored ||
        isTimelockedStakedObjectsErrored ||
        isTimelockedObjectsError;

    const BALANCE_BREAKDOWN: BalanceBreakdownElement[] = [
        {
            keyText: 'Liquid Balance',
            value: totalLiquidBalance,
            supportingLabel: symbol,
            isLoading: isLoadingBalance,
            isError: isBalanceErrored,
        },
        {
            keyText: 'Staked',
            value: formattedDelegatedStake,
            supportingLabel: symbol,
            isLoading: isLoadingDelegatedStakes,
            isError: isDelegatedStakeErrored,
        },
        {
            keyText: 'Timelocked Staked',
            value: formattedTimelockedStake,
            supportingLabel: symbol,
            isLoading: isLoadingTimelockedStakeObjects,
            isError: isTimelockedStakedObjectsErrored,
        },
        {
            keyText: 'Timelocked',
            value: formattedTimelockedTokens,
            supportingLabel: symbol,
            isLoading: isTimelockedObjectsLoading,
            isError: isTimelockedObjectsError,
        },
        {
            keyText: 'Total',
            value: formattedTotalBalance,
            supportingLabel: symbol,
            isLoading: isLoadingTotalBalance,
            isError: isTotalBalanceErrored,
        },
    ];
    return (
        <>
            {BALANCE_BREAKDOWN.map((item) => (
                <KeyValueInfo
                    key={item.keyText}
                    keyText={item.keyText}
                    fullwidth
                    value={
                        <RenderBalanceValue
                            value={item.value}
                            isLoading={item.isLoading}
                            isError={item.isError}
                        />
                    }
                    supportingLabel={item.supportingLabel}
                />
            ))}
        </>
    );
}

interface RenderBalanceValueProps {
    value: string;
    isLoading?: boolean;
    isError?: boolean;
}

function RenderBalanceValue({
    value,
    isLoading,
    isError,
}: RenderBalanceValueProps): React.JSX.Element | string {
    if (isLoading) {
        return <Skeleton widthClass="w-20" heightClass="h-4" />;
    }
    if (isError) {
        return '--';
    }

    return value;
}
