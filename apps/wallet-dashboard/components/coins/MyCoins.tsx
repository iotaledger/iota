// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { useCurrentAccount, useIotaClientQuery } from '@iota/dapp-kit';
import { CoinBalance } from '@iota/iota-sdk/client';
import {
    COINS_QUERY_REFETCH_INTERVAL,
    COINS_QUERY_STALE_TIME,
    filterAndSortTokenBalances,
    useSortedCoinsByCategories,
    CoinItem,
} from '@iota/core';
import {
    ButtonSegment,
    Panel,
    SegmentedButton,
    SegmentedButtonType,
    Title,
} from '@iota/apps-ui-kit';
import { RecognizedBadge } from '@iota/ui-icons';
import { SendTokenDialog } from '@/components';

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
    const [isSendTokenDialogOpen, setIsSendTokenDialogOpen] = useState(false);
    const [selectedCoin, setSelectedCoin] = useState<CoinBalance>();

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

    function openSendTokenDialog(coin: CoinBalance): void {
        if (coinBalances) {
            setIsSendTokenDialogOpen(true);
            setSelectedCoin(coin);
        }
    }

    return (
        <Panel>
            <div className="flex h-full w-full flex-col">
                <Title title="My Coins" />
                <div className="px-sm pt-sm">
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
                </div>
                <div className="h-full overflow-y-auto px-sm pb-md pt-sm">
                    {[TokenCategory.All, TokenCategory.Recognized].includes(
                        selectedTokenCategory,
                    ) &&
                        recognized?.map((coin, index) => {
                            return (
                                <CoinItem
                                    key={index}
                                    coinType={coin.coinType}
                                    balance={BigInt(coin.totalBalance)}
                                    onClick={() => openSendTokenDialog(coin)}
                                    icon={<RecognizedBadge className="h-4 w-4 text-primary-40" />}
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
                                    onClick={() => openSendTokenDialog(coin)}
                                />
                            );
                        })}
                </div>
            </div>
            {selectedCoin && activeAccountAddress && (
                <SendTokenDialog
                    activeAddress={activeAccountAddress}
                    coin={selectedCoin}
                    open={isSendTokenDialogOpen}
                    setOpen={setIsSendTokenDialogOpen}
                />
            )}
        </Panel>
    );
}

export default MyCoins;
