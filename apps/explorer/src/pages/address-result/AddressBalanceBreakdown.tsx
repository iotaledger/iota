// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Collapsible,
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
import { Divider, KeyValueInfo, Panel, Skeleton, Title, TitleSize } from '@iota/apps-ui-kit';
import { useState } from 'react';

const TOOLTIP_TEXT = 'This balance breakdown does not include unmigrated stardust funds.';
interface BalanceBreakdownElement {
    keyText: string;
    value: string;
    supportingLabel: string;
    isLoading?: boolean;
    isError?: boolean;
    tooltipText?: string;
}

export function AddressBalanceBreakdown({ address }: { address: string }): React.JSX.Element {
    const [open, setOpen] = useState(true);
    const {
        data: balance,
        isLoading: isLoadingBalance,
        isError: isBalanceErrored,
    } = useBalance(address);

    const [totalAvailableBalance, symbol] = useFormatCoin({
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

    const totalTimelockedStaked = useTotalDelegatedStake(extendedDelegatedTimelockedStakes);
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
            keyText: 'Available',
            value: totalAvailableBalance,
            supportingLabel: symbol,
            isLoading: isLoadingBalance,
            isError: isBalanceErrored,
            tooltipText: 'IOTA that can be used or transferred immediately.',
        },
        {
            keyText: 'Staked',
            value: formattedDelegatedStake,
            supportingLabel: symbol,
            isLoading: isLoadingDelegatedStakes,
            isError: isDelegatedStakeErrored,
            tooltipText: 'IOTA currently locked in staking. Cannot be used until unstaked.',
        },
        {
            keyText: 'Timelocked Staked',
            value: formattedTimelockedStake,
            supportingLabel: symbol,
            isLoading: isLoadingTimelockedStakeObjects,
            isError: isTimelockedStakedObjectsErrored,
            tooltipText:
                'IOTA both timelocked and staked. To access these funds, they must first be unstaked, and then handled according to their timelock conditions.',
        },
        {
            keyText: 'Timelocked',
            value: formattedTimelockedTokens,
            supportingLabel: symbol,
            isLoading: isTimelockedObjectsLoading,
            isError: isTimelockedObjectsError,
            tooltipText:
                "IOTA locked until a specific time. Depending on the lock's expiration, these funds can either be used for staking or collected when the timelock allows it.",
        },
    ];
    return (
        <Panel>
            <div className="relative overflow-visible">
                <Collapsible
                    hideBorder
                    isOpen={open}
                    onOpenChange={(isOpen) => setOpen(isOpen)}
                    render={() => (
                        <div className="flex w-full flex-row items-center justify-between">
                            <Title
                                size={TitleSize.Small}
                                title="Balance Breakdown"
                                tooltipText={TOOLTIP_TEXT}
                            />
                        </div>
                    )}
                >
                    <div className="flex flex-col gap-y-sm p-md--rs">
                        {BALANCE_BREAKDOWN.map((item) => (
                            <KeyValueInfo
                                key={item.keyText}
                                keyText={item.keyText}
                                tooltipText={item.tooltipText}
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
                    </div>
                </Collapsible>
                <div className="flex flex-col gap-y-sm px-md pb-md">
                    <Divider />
                    <KeyValueInfo
                        keyText="Total"
                        value={
                            <RenderBalanceValue
                                value={formattedTotalBalance}
                                isLoading={isLoadingTotalBalance}
                                isError={isTotalBalanceErrored}
                            />
                        }
                        fullwidth
                        supportingLabel={symbol}
                    />
                </div>
            </div>
        </Panel>
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
