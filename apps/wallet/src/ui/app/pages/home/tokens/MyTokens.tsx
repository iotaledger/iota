// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SegmentedButton, SegmentedButtonType, ButtonSegment, Title } from '@iota/apps-ui-kit';
import { useSortedCoinsByCategories } from '@iota/core';
import { RecognizedBadge } from '@iota/apps-ui-icons';
import { ampli } from '_src/shared/analytics/ampli';
import { Loading } from '_src/ui/app/components';
import { usePinnedCoinTypes } from '_hooks';
import { useState } from 'react';
import { PinButton } from './PinButton';
import { TokenLink } from './TokenLink';
import { type CoinBalance } from '@iota/iota-sdk/client';

interface MyTokensProps {
    coinBalances: CoinBalance[];
    isLoading: boolean;
    isFetched: boolean;
}

enum TokenCategory {
    All = 'All',
    Recognized = 'Recognized',
    Unrecognized = 'Unrecognized',
}

const TOKEN_CATEGORIES = [
    {
        label: 'All',
        value: TokenCategory.All,
    },
    {
        label: 'Recognized',
        value: TokenCategory.Recognized,
    },
    {
        label: 'Unrecognized',
        value: TokenCategory.Unrecognized,
    },
];

export function MyTokens({ coinBalances, isLoading, isFetched }: MyTokensProps) {
    const [selectedTokenCategory, setSelectedTokenCategory] = useState(TokenCategory.All);

    const [_pinned, { pinCoinType, unpinCoinType }] = usePinnedCoinTypes();

    const { recognized, pinned, unrecognized } = useSortedCoinsByCategories(coinBalances, _pinned);

    // Avoid perpetual loading state when fetching and retry keeps failing; add isFetched check.
    const isFirstTimeLoading = isLoading && !isFetched;

    function handlePin(coinType: string) {
        ampli.pinnedCoin({
            coinType: coinType,
        });
        pinCoinType(coinType);
    }

    function handleUnpin(coinType: string) {
        ampli.unpinnedCoin({
            coinType: coinType,
        });
        unpinCoinType(coinType);
    }

    return (
        <Loading loading={isFirstTimeLoading}>
            <div className="w-full">
                <div className="flex h-[56px] items-center">
                    <Title title="My coins" />
                </div>
                <div className="inline-flex">
                    <SegmentedButton type={SegmentedButtonType.Filled}>
                        {TOKEN_CATEGORIES.map(({ label, value }) => {
                            const recognizedButEmpty =
                                value === TokenCategory.Recognized ? !recognized.length : false;
                            const notRecognizedButEmpty =
                                value === TokenCategory.Unrecognized
                                    ? !pinned?.length && !unrecognized?.length
                                    : false;

                            return (
                                <ButtonSegment
                                    key={value}
                                    onClick={() => setSelectedTokenCategory(value)}
                                    label={label}
                                    selected={selectedTokenCategory === value}
                                    disabled={recognizedButEmpty || notRecognizedButEmpty}
                                />
                            );
                        })}
                    </SegmentedButton>
                </div>
                <div className="pb-md pt-sm">
                    {[TokenCategory.All, TokenCategory.Recognized].includes(
                        selectedTokenCategory,
                    ) &&
                        recognized.map((coinBalance) => (
                            <TokenLink
                                key={coinBalance.coinType}
                                coinBalance={coinBalance}
                                icon={<RecognizedBadge className="h-4 w-4 text-primary-40" />}
                            />
                        ))}
                    {[TokenCategory.All, TokenCategory.Unrecognized].includes(
                        selectedTokenCategory,
                    ) &&
                        pinned.map((coinBalance) => (
                            <TokenLink
                                key={coinBalance.coinType}
                                coinBalance={coinBalance}
                                clickableAction={
                                    <PinButton
                                        isPinned
                                        onClick={() => handleUnpin(coinBalance.coinType)}
                                    />
                                }
                            />
                        ))}

                    {[TokenCategory.All, TokenCategory.Unrecognized].includes(
                        selectedTokenCategory,
                    ) &&
                        unrecognized.map((coinBalance) => (
                            <TokenLink
                                key={coinBalance.coinType}
                                coinBalance={coinBalance}
                                clickableAction={
                                    <PinButton onClick={() => handlePin(coinBalance.coinType)} />
                                }
                            />
                        ))}
                </div>
            </div>
        </Loading>
    );
}
