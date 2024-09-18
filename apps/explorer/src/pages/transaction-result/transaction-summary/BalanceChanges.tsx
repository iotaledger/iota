// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Card,
    CardAction,
    CardActionType,
    CardBody,
    CardImage,
    CardType,
    ImageType,
} from '@iota/apps-ui-kit';
import {
    type BalanceChange,
    type BalanceChangeSummary,
    CoinFormat,
    getRecognizedUnRecognizedTokenChanges,
    useCoinMetadata,
    useFormatCoin,
} from '@iota/core';
import { RecognizedBadge } from '@iota/ui-icons';
import clsx from 'clsx';
import { useMemo } from 'react';
import { AddressLink, Coin, CollapsibleCard, CollapsibleSection } from '~/components/ui';

interface BalanceChangesProps {
    changes: BalanceChangeSummary;
}

function BalanceChangeEntry({ change }: { change: BalanceChange }): JSX.Element | null {
    const { amount, coinType, recipient, unRecognizedToken } = change;
    const [formatted, symbol] = useFormatCoin(amount, coinType, CoinFormat.FULL);
    const { data: coinMetaData } = useCoinMetadata(coinType);
    const isPositive = BigInt(amount) > 0n;

    if (!change) {
        return null;
    }

    return (
        <div className="flex flex-col gap-xs">
            <Card type={CardType.Filled}>
                <CardImage type={ImageType.BgTransparent}>
                    <Coin type={coinType} />
                </CardImage>
                <CardBody
                    title={coinMetaData?.name || symbol}
                    icon={
                        !unRecognizedToken ? (
                            <RecognizedBadge className="h-4 w-4 text-primary-40" />
                        ) : null
                    }
                />
                <CardAction
                    type={CardActionType.SupportingText}
                    title={`${isPositive ? '+' : ''} ${formatted} ${symbol}`}
                />
            </Card>
            {recipient && (
                <div className="flex flex-wrap items-center justify-between px-sm py-xs">
                    <span className="w-full flex-shrink-0 text-label-lg text-neutral-40 dark:text-neutral-60 md:w-40">
                        Recipient
                    </span>
                    <AddressLink address={recipient} />
                </div>
            )}
        </div>
    );
}

function BalanceChangeCard({ changes, owner }: { changes: BalanceChange[]; owner: string }) {
    const { recognizedTokenChanges, unRecognizedTokenChanges } = useMemo(
        () => getRecognizedUnRecognizedTokenChanges(changes),
        [changes],
    );

    return (
        <CollapsibleCard
            title="Balance Changes"
            footer={
                owner ? (
                    <div className="flex flex-wrap justify-between px-md--rs py-sm--rs">
                        <span className="text-body-md text-neutral-40 dark:text-neutral-60">
                            Owner
                        </span>
                        <AddressLink label={undefined} address={owner} />
                    </div>
                ) : null
            }
        >
            <div className="flex flex-col gap-2">
                {recognizedTokenChanges.map((change, index) => (
                    <CollapsibleSection key={index + change.coinType} hideBorder>
                        <BalanceChangeEntry change={change} />
                    </CollapsibleSection>
                ))}
                {unRecognizedTokenChanges.length > 0 && (
                    <div
                        className={clsx(
                            'flex flex-col gap-2',
                            recognizedTokenChanges?.length && 'border-t border-gray-45 pt-2',
                        )}
                    >
                        {unRecognizedTokenChanges.map((change, index) => (
                            <CollapsibleSection key={index + change.coinType} hideBorder>
                                <BalanceChangeEntry change={change} />
                            </CollapsibleSection>
                        ))}
                    </div>
                )}
            </div>
        </CollapsibleCard>
    );
}

export function BalanceChanges({ changes }: BalanceChangesProps) {
    if (!changes) return null;

    return (
        <>
            {Object.entries(changes).map(([owner, changes]) => (
                <BalanceChangeCard key={owner} changes={changes} owner={owner} />
            ))}
        </>
    );
}
