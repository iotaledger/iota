// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin } from '@iota/core';
import { CheckmarkFilled, LockUnlocked, Stake } from '@iota/apps-ui-icons';
import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { InfoBox, InfoBoxStyle, InfoBoxType, ListItem } from '@iota/apps-ui-kit';

interface CollectSummaryProps {
    transaction: IotaTransactionBlockResponse;
    activeAddress: string;
}

interface SummaryItem {
    icon: React.ReactNode;
    text: string;
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
        .filter((change) => change.coinType === '0x2::iota::IOTA' && BigInt(change.amount) > 0n)
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

    // Count normal stakes created/modified
    const stakesCreated = objectChanges.filter(
        (change) =>
            (change.type === 'created' || change.type === 'mutated') &&
            change.objectType.includes('::staking_pool::StakedIota') &&
            !change.objectType.includes('Timelocked'),
    ).length;

    const stakesMerged =
        timelockStakesConverted > stakesCreated ? timelockStakesConverted - stakesCreated : 0;

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

    // Build list items with detailed explanations
    const items: SummaryItem[] = [];

    if (timelocksUnlocked > 0) {
        items.push({
            icon: <LockUnlocked className="text-primary-40 h-4 w-4" />,
            text: `${timelocksUnlocked} Timelock${timelocksUnlocked > 1 ? 's have' : ' has'} been unlocked. You received ${formattedReceived} ${receivedSymbol} directly to your wallet`,
        });
    }

    if (timelockStakesConverted > 0) {
        if (stakesMerged > 0) {
            items.push({
                icon: <Stake className="text-primary-40 h-4 w-4" />,
                text: `${timelockStakesConverted} Timelock Stake${timelockStakesConverted > 1 ? 's have' : ' has'} been converted to regular Stakes. ${stakesMerged} ${stakesMerged > 1 ? 'were' : 'was'} automatically merged with your existing stakes to the same validator${stakesMerged > 1 ? 's' : ''}`,
            });
        } else {
            items.push({
                icon: <Stake className="text-primary-40 h-4 w-4" />,
                text: `${timelockStakesConverted} Timelock Stake${timelockStakesConverted > 1 ? 's have' : ' has'} been converted to regular Stakes. ${timelockStakesConverted > 1 ? 'They are' : 'It is'} now actively earning rewards with your selected validator${timelockStakesConverted > 1 ? 's' : ''}`,
            });
        }
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

            {items.length > 0 && (
                <div className="flex flex-col">
                    {items.map((item, index) => (
                        <ListItem key={index} hideBottomBorder={index === items.length - 1}>
                            <div className="flex items-start gap-xs">
                                <div className="mt-0.5">{item.icon}</div>
                                <span className="text-neutral-10 dark:text-neutral-92 text-body-md">
                                    {item.text}
                                </span>
                            </div>
                        </ListItem>
                    ))}
                </div>
            )}
        </div>
    );
}
