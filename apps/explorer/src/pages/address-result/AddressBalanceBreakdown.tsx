// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Collapsible,
    formatDelegatedStake,
    formatDelegatedTimelockedStake,
    IOTA_COIN_METADATA,
    mapTimelockObjects,
    TIMELOCK_IOTA_TYPE,
    useBalance,
    useFormatCoin,
    useGetAllOwnedObjects,
    useGetDelegatedStake,
    useGetTimelockedStakedObjects,
    useTotalDelegatedStake,
} from '@iota/core';
import {
    Divider,
    KeyValueInfo,
    Panel,
    Skeleton,
    Title,
    TitleSize,
    Tooltip,
} from '@iota/apps-ui-kit';
import { useState } from 'react';
import { CoinFormat, formatBalance } from '@iota/iota-sdk/utils';
import { onCopySuccess } from '~/lib';

const TOOLTIP_TEXT = 'This balance breakdown does not include unmigrated stardust funds.';
interface BalanceBreakdownElement {
    keyText: string;
    value: bigint;
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
    const totalAvailableBalance = balance?.totalBalance ? BigInt(balance.totalBalance) : BigInt(0);

    const {
        data: delegatedStake,
        isLoading: isLoadingDelegatedStakes,
        isError: isDelegatedStakeErrored,
    } = useGetDelegatedStake({
        address,
    });
    const delegatedStakes = delegatedStake ? formatDelegatedStake(delegatedStake) : [];
    const totalDelegatedStake = useTotalDelegatedStake(delegatedStakes);

    const {
        data: timelockedStakedObjects,
        isLoading: isLoadingTimelockedStakeObjects,
        isError: isTimelockedStakedObjectsErrored,
    } = useGetTimelockedStakedObjects(address);

    const extendedDelegatedTimelockedStakes = formatDelegatedTimelockedStake(
        timelockedStakedObjects || [],
    );

    const totalTimelockedStaked = useTotalDelegatedStake(extendedDelegatedTimelockedStakes);

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

    const totalBalanceBreakdown =
        BigInt(balance?.totalBalance || 0) +
        BigInt(totalDelegatedStake || 0) +
        BigInt(totalTimelockedStaked || 0) +
        BigInt(totalTimelockedTokens || 0);

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
            isLoading: isLoadingBalance,
            isError: isBalanceErrored,
            tooltipText: 'IOTA that can be used or transferred immediately.',
        },
        {
            keyText: 'Staked',
            value: totalDelegatedStake,
            isLoading: isLoadingDelegatedStakes,
            isError: isDelegatedStakeErrored,
            tooltipText: 'IOTA currently locked in staking. Cannot be used until unstaked.',
        },
        {
            keyText: 'Timelocked Staked',
            value: totalTimelockedStaked,
            isLoading: isLoadingTimelockedStakeObjects,
            isError: isTimelockedStakedObjectsErrored,
            tooltipText:
                'IOTA both timelocked and staked. To access these funds, they must first be unstaked, and then handled according to their timelock conditions.',
        },
        {
            keyText: 'Timelocked',
            value: totalTimelockedTokens,
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
                                supportingLabel={IOTA_COIN_METADATA.symbol}
                                copyText={formatBalance(
                                    item.value,
                                    IOTA_COIN_METADATA.decimals,
                                    CoinFormat.Full,
                                )}
                                onCopySuccess={onCopySuccess}
                            />
                        ))}
                    </div>
                </Collapsible>
                <div className="flex flex-col gap-y-sm px-md pb-md md:px-lg">
                    <Divider />
                    <KeyValueInfo
                        keyText="Total"
                        value={
                            <RenderBalanceValue
                                value={totalBalanceBreakdown}
                                isLoading={isLoadingTotalBalance}
                                isError={isTotalBalanceErrored}
                            />
                        }
                        fullwidth
                        supportingLabel={IOTA_COIN_METADATA.symbol}
                        copyText={formatBalance(
                            totalBalanceBreakdown,
                            IOTA_COIN_METADATA.decimals,
                            CoinFormat.Full,
                        )}
                        onCopySuccess={onCopySuccess}
                    />
                </div>
            </div>
        </Panel>
    );
}

interface RenderBalanceValueProps {
    value: bigint;
    isLoading?: boolean;
    isError?: boolean;
}

function RenderBalanceValue({
    value,
    isLoading,
    isError,
}: RenderBalanceValueProps): React.JSX.Element | string {
    const [roundedAmount] = useFormatCoin({
        balance: value,
    });
    const [fullAmount, symbol] = useFormatCoin({
        balance: value,
        format: CoinFormat.Full,
    });
    if (isLoading) {
        return <Skeleton className="h-4 w-20" />;
    }
    if (isError) {
        return '--';
    }

    return (
        <Tooltip openDelay={100} text={`${fullAmount} ${symbol}`}>
            <span>{roundedAmount}</span>
        </Tooltip>
    );
}
