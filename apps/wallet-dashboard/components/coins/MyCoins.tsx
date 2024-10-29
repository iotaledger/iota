// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { useCurrentAccount, useIotaClientQuery } from '@iota/dapp-kit';
import { CoinItem, SendCoinPopup } from '@/components';
import { usePopups } from '@/hooks';
import { CoinBalance } from '@iota/iota-sdk/client';
import {
    COINS_QUERY_REFETCH_INTERVAL,
    COINS_QUERY_STALE_TIME,
    filterAndSortTokenBalances,
    useSortedCoinsByCategories,
} from '@iota/core';
import {
    ButtonSegment,
    Panel,
    SegmentedButton,
    SegmentedButtonType,
    Title,
} from '@iota/apps-ui-kit';
import { RecognizedBadge } from '@iota/ui-icons';

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

function MyCoins(): React.JSX.Element {
    const [selectedTokenCategory, setSelectedTokenCategory] = useState(TokenCategory.All);

    const { openPopup, closePopup } = usePopups();
    const account = useCurrentAccount();
    const activeAccountAddress = account?.address;

    const { data: coinBalances } = useIotaClientQuery(
        'getAllBalances',
        { owner: activeAccountAddress! },
        {
            enabled: !!activeAccountAddress,
            staleTime: COINS_QUERY_STALE_TIME,
            refetchInterval: COINS_QUERY_REFETCH_INTERVAL,
            select: filterAndSortTokenBalances,
        },
    );
    const { recognized, unrecognized } = useSortedCoinsByCategories(coinBalances ?? []);

    function openSendTokenPopup(coin: CoinBalance, address: string): void {
        if (coinBalances) {
            openPopup(
                <SendCoinPopup
                    coin={coin}
                    senderAddress={address}
                    onClose={closePopup}
                    coins={coinBalances}
                />,
            );
        }
    }

    return (
        <Panel>
            <div className="flex w-full flex-col">
                <Title title="My Coins" />
                <div className="px-sm pb-md pt-sm">
                    <div className="inline-flex">
                        <SegmentedButton type={SegmentedButtonType.Filled}>
                            {TOKEN_CATEGORIES.map(({ label, value }) => {
                                const recognizedButEmpty =
                                    value === TokenCategory.Recognized ? !recognized.length : false;
                                const notRecognizedButEmpty =
                                    value === TokenCategory.Unrecognized
                                        ? !unrecognized?.length
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
                            recognized?.map((coin, index) => {
                                return (
                                    <CoinItem
                                        key={index}
                                        coinType={coin.coinType}
                                        balance={BigInt(coin.totalBalance)}
                                        onClick={() =>
                                            openSendTokenPopup(coin, account?.address ?? '')
                                        }
                                        icon={
                                            <RecognizedBadge className="h-4 w-4 text-primary-40" />
                                        }
                                    />
                                );
                            })}
                        {[TokenCategory.All, TokenCategory.Unrecognized].includes(
                            selectedTokenCategory,
                        ) &&
                            unrecognized?.map((coin, index) => {
                                return (
                                    <CoinItem
                                        key={index}
                                        coinType={coin.coinType}
                                        balance={BigInt(coin.totalBalance)}
                                        onClick={() =>
                                            openSendTokenPopup(coin, account?.address ?? '')
                                        }
                                    />
                                );
                            })}
                    </div>
                </div>
            </div>
        </Panel>
    );
}

export default MyCoins;
