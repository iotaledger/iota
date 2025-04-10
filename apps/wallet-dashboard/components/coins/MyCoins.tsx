// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { useCurrentAccount } from '@iota/dapp-kit';
import { CoinBalance } from '@iota/iota-sdk/client';
import { useSortedCoinsByCategories, CoinItem, useGetAllBalances, VirtualList } from '@iota/core';
import {
    ButtonSegment,
    Panel,
    SegmentedButton,
    SegmentedButtonType,
    Title,
} from '@iota/apps-ui-kit';
import { RecognizedBadge } from '@iota/apps-ui-icons';
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

export function MyCoins(): React.JSX.Element {
    const [selectedTokenCategory, setSelectedTokenCategory] = useState(TokenCategory.All);
    const [isSendTokenDialogOpen, setIsSendTokenDialogOpen] = useState(false);
    const [selectedCoin, setSelectedCoin] = useState<CoinBalance>();

    const account = useCurrentAccount();
    const activeAccountAddress = account?.address;

    const { data: coinBalances } = useGetAllBalances(activeAccountAddress);
    const { recognized, unrecognized } = useSortedCoinsByCategories(coinBalances ?? []);

    function openSendTokenDialog(coin: CoinBalance): void {
        if (coinBalances) {
            setIsSendTokenDialogOpen(true);
            setSelectedCoin(coin);
        }
    }

    const virtualItem = (isRecognized: boolean, coin: CoinBalance): JSX.Element => {
        return (
            <CoinItem
                coinType={coin.coinType}
                balance={BigInt(coin.totalBalance)}
                onClick={() => openSendTokenDialog(coin)}
                icon={isRecognized ? <RecognizedBadge className="h-4 w-4 text-primary-40" /> : null}
            />
        );
    };
    return (
        <Panel>
            <div className="flex h-full w-full flex-col">
                <Title title="My Coins" />
                <div className="px-sm py-sm md:px-xxs lg:px-sm">
                    <div className="inline-flex w-full justify-start md:justify-center lg:justify-start">
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
                <div className="max-h-[400px] flex-1 overflow-y-auto px-sm pb-md pt-sm sm:max-h-none">
                    <VirtualList
                        items={
                            selectedTokenCategory === TokenCategory.Recognized
                                ? recognized
                                : selectedTokenCategory === TokenCategory.Unrecognized
                                  ? unrecognized
                                  : [...recognized!, ...unrecognized!]
                        }
                        estimateSize={() => 60}
                        render={(coin: CoinBalance) => {
                            const isRecognized = recognized?.find(
                                (c) => c.coinType === coin.coinType,
                            );
                            return virtualItem(!!isRecognized, coin);
                        }}
                        heightClassName="h-full"
                    />
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
