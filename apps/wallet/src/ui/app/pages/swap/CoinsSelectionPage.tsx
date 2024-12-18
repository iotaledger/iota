// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetAllBalances } from '_app/hooks/useGetAllBalances';
import { useValidSwapTokensList } from '_app/hooks/useValidSwapTokensList';
import Loading from '_components/loading';
import Overlay from '_components/overlay';
import { useActiveAddress, useSortedCoinsByCategories } from '_hooks';
import { TokenRow } from '_pages/home/tokens/TokensDetails';
import { normalizeStructTag } from '@iota/iota-sdk/utils';
import { Fragment } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';

export function CoinsSelectionPage() {
    const navigate = useNavigate();
    const selectedAddress = useActiveAddress();
    const [searchParams] = useSearchParams();
    const fromCoinType = searchParams.get('fromCoinType');
    const toCoinType = searchParams.get('toCoinType');
    const source = searchParams.get('source');
    const currentAmount = searchParams.get('currentAmount');

    const { data: swapFromTokensList, isLoading } = useValidSwapTokensList();
    const swapToTokensList = swapFromTokensList.filter((token) => {
        if (!fromCoinType) {
            return true;
        }
        return normalizeStructTag(token) !== normalizeStructTag(fromCoinType);
    });

    const allowedCoinTypes = source === 'fromCoinType' ? swapFromTokensList : swapToTokensList;

    const { data: coinBalances, isPending } = useGetAllBalances(selectedAddress || '');

    const { recognized } = useSortedCoinsByCategories(coinBalances ?? []);

    return (
        <Overlay showModal title="Select a Coin" closeOverlay={() => navigate(-1)}>
            <Loading loading={isPending || isLoading}>
                <div className="flex w-full flex-shrink-0 flex-col justify-start">
                    {allowedCoinTypes.map((coinType, index) => {
                        const coinBalance =
                            recognized?.find((coin) => coin.coinType === coinType) || {};
                        const totalBalance =
                            coinBalances?.find(
                                (balance) =>
                                    normalizeStructTag(balance.coinType) ===
                                    normalizeStructTag(coinType),
                            )?.totalBalance ?? '0';

                        return (
                            <Fragment key={coinType}>
                                <TokenRow
                                    coinBalance={{
                                        coinType,
                                        coinObjectCount: 0,
                                        lockedBalance: {},
                                        totalBalance,
                                        ...coinBalance,
                                    }}
                                    onClick={() => {
                                        const params = fromCoinType
                                            ? {
                                                  type: fromCoinType,
                                                  toType: coinType,
                                                  presetAmount: currentAmount || '0',
                                              }
                                            : {
                                                  type: coinType,
                                                  toType: toCoinType || '',
                                                  presetAmount: currentAmount || '0',
                                              };
                                        navigate(`/swap?${new URLSearchParams(params)}`);
                                    }}
                                />

                                <div className="h-px w-full bg-gray-45" />
                            </Fragment>
                        );
                    })}
                </div>
            </Loading>
        </Overlay>
    );
}
