// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin } from '@iota/core';
import { CheckmarkFilled, LockUnlocked, Stake } from '@iota/apps-ui-icons';
import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { InfoBox, InfoBoxStyle, InfoBoxType, ListItem } from '@iota/apps-ui-kit';

interface CollectSummaryProps {
    transaction: IotaTransactionBlockResponse;
    activeAddress: string;
}

interface SummaryItem {
    icon: React.ReactNode;
    title: string;
    description: string;
}

export function CollectSummary({ transaction, activeAddress }: CollectSummaryProps) {
    const balanceChanges = transaction.balanceChanges || [];
    const userBalanceChanges = balanceChanges.filter((change) => {
        const owner = change.owner;
        if (typeof owner === 'object' && owner !== null && 'AddressOwner' in owner) {
            return owner.AddressOwner === activeAddress;
        }
        return false;
    });

    const iotaReceived = userBalanceChanges
        .filter((change) => change.coinType === IOTA_TYPE_ARG && BigInt(change.amount) > 0n)
        .reduce((sum, change) => sum + BigInt(change.amount), 0n);

    const [formattedReceived, receivedSymbol] = useFormatCoin({ balance: iotaReceived });

    const objectChanges = transaction.objectChanges || [];

    // Count unlocked timelocks (deleted objects of type timelock)
    const timelocksUnlocked = objectChanges.filter(
        (change) => change.type === 'deleted' && change.objectType.includes('::timelock::TimeLock'),
    ).length;

    // Count converted timelock stakes (deleted objects of type timelocked_staking)
    const timelockStakesConverted = objectChanges.filter(
        (change) =>
            change.type === 'deleted' &&
            change.objectType.includes('::timelocked_staking::TimelockedStakedIota'),
    ).length;

    // Build description
    const collectedItems: string[] = [];

    if (timelocksUnlocked > 0) {
        collectedItems.push(`${timelocksUnlocked} Timelock${timelocksUnlocked > 1 ? 's' : ''}`);
    }

    if (timelockStakesConverted > 0) {
        collectedItems.push(
            `${timelockStakesConverted} Timelock Stake${timelockStakesConverted > 1 ? 's' : ''}`,
        );
    }

    const collectedDescription =
        collectedItems.length > 0
            ? `You collected ${collectedItems.join(' and ')}`
            : 'Your collection was completed successfully';

    const summaryItems: SummaryItem[] = [];

    if (timelocksUnlocked > 0) {
        summaryItems.push({
            icon: <LockUnlocked className="text-primary-40 h-4 w-4" />,
            title: `${timelocksUnlocked} Timelock${timelocksUnlocked > 1 ? 's' : ''} Unlocked`,
            description: `${formattedReceived} ${receivedSymbol} added to your wallet`,
        });
    }

    if (timelockStakesConverted > 0) {
        summaryItems.push({
            icon: <Stake className="text-primary-40 h-4 w-4" />,
            title: `${timelockStakesConverted} Timelock Restake${timelockStakesConverted > 1 ? 's' : ''}`,
            description: `Converted to regular Stake${timelockStakesConverted > 1 ? 's' : ''} of the same validator`,
        });
    }

    return (
        <div className="flex flex-col gap-md">
            <InfoBox
                type={InfoBoxType.Success}
                style={InfoBoxStyle.Elevated}
                title="Success!"
                supportingText={collectedDescription}
                icon={<CheckmarkFilled className="text-success" />}
            />

            {summaryItems.length > 0 && (
                <div className="flex flex-col">
                    {summaryItems.map((item, index) => (
                        <ListItem key={index} hideBottomBorder={index === summaryItems.length - 1}>
                            <div className="flex items-start gap-xs">
                                <div className="mt-0.5">{item.icon}</div>
                                <div className="flex flex-col gap-xxs">
                                    <span className="text-neutral-10 dark:text-neutral-92 text-body-md font-semibold">
                                        {item.title}
                                    </span>
                                    <span className="text-body-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                                        {item.description}
                                    </span>
                                </div>
                            </div>
                        </ListItem>
                    ))}
                </div>
            )}
        </div>
    );
}
