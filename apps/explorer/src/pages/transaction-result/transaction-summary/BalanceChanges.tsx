// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Accordion,
    AccordionContent,
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
    ImageIconSize,
    CoinIcon,
} from '@iota/core';
import { RecognizedBadge } from '@iota/apps-ui-icons';
import { useMemo } from 'react';
import { AddressLink, CollapsibleCard } from '~/components/ui';
import { BREAK_POINT, useMediaQuery } from '~/hooks';

interface BalanceChangesProps {
    changes: BalanceChangeSummary;
}

function BalanceChangeEntry({ change }: { change: BalanceChange }): JSX.Element | null {
    const { amount, coinType, recipient, unRecognizedToken } = change;
    const isMdScreen = useMediaQuery(
        `(min-width: ${BREAK_POINT.md}px) and (max-width: ${BREAK_POINT.lg - 1}px)`,
    );
    const coinFormat = isMdScreen ? CoinFormat.ROUNDED : CoinFormat.FULL;
    const [formatted, symbol] = useFormatCoin(amount, coinType, coinFormat);
    const { data: coinMetaData } = useCoinMetadata(coinType);
    const isPositive = BigInt(amount) > 0n;

    if (!change) {
        return null;
    }

    return (
        <div className="flex flex-col gap-xs">
            <Card type={CardType.Filled}>
                <CardImage type={ImageType.BgTransparent}>
                    <CoinIcon coinType={coinType} size={ImageIconSize.Small} />
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
                    <span className="w-full flex-shrink-0 text-label-lg text-neutral-40 md:w-40 dark:text-neutral-60">
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
            isTransparentPanel
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
            <div className="flex flex-col gap-md px-md--rs py-sm">
                {recognizedTokenChanges.map((change, index) => (
                    <div key={index + change.coinType}>
                        <Accordion>
                            <AccordionContent isExpanded>
                                <BalanceChangeEntry change={change} />
                            </AccordionContent>
                        </Accordion>
                    </div>
                ))}
                {unRecognizedTokenChanges.length > 0 && (
                    <div className="flex flex-col gap-md">
                        {unRecognizedTokenChanges.map((change, index) => (
                            <div key={index + change.coinType}>
                                <Accordion hideBorder>
                                    <AccordionContent isExpanded>
                                        <BalanceChangeEntry change={change} />
                                    </AccordionContent>
                                </Accordion>
                            </div>
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
