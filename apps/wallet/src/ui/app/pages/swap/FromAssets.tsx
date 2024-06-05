// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import Loading from '_components/loading';
import Overlay from '_components/overlay';
import { filterAndSortTokenBalances } from '_helpers';
import {
    useActiveAddress,
    useAllowedSwapCoinsList,
    useCoinsReFetchingConfig,
    useSortedCoinsByCategories,
} from '_hooks';
import { TokenRow } from '_pages/home/tokens/TokensDetails';
import { DeepBookContextProvider } from '_shared/deepBook/context';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { Fragment } from 'react';
import { useNavigate } from 'react-router-dom';

function FromAssetsComponent() {
    const navigate = useNavigate();
    const selectedAddress = useActiveAddress();
    const { staleTime, refetchInterval } = useCoinsReFetchingConfig();

    const { data: coins, isPending } = useIotaClientQuery(
        'getAllBalances',
        { owner: selectedAddress! },
        {
            enabled: !!selectedAddress,
            refetchInterval,
            staleTime,
            select: filterAndSortTokenBalances,
        },
    );

    const { recognized } = useSortedCoinsByCategories(coins ?? []);
    const allowedSwapCoinsList = useAllowedSwapCoinsList();

    const renderedRecognizedCoins = recognized.filter(({ coinType }) =>
        allowedSwapCoinsList.includes(coinType),
    );

    return (
        <Overlay showModal title="Select a Coin" closeOverlay={() => navigate(-1)}>
            <Loading loading={isPending}>
                <div className="flex w-full flex-shrink-0 flex-col justify-start">
                    {renderedRecognizedCoins?.map((coinBalance, index) => {
                        return (
                            <Fragment key={coinBalance.coinType}>
                                <TokenRow
                                    coinBalance={coinBalance}
                                    onClick={() => {
                                        navigate(
                                            `/swap?${new URLSearchParams({
                                                type: coinBalance.coinType,
                                            }).toString()}`,
                                        );
                                    }}
                                />

                                {index !== recognized.length - 1 && (
                                    <div className="h-px w-full bg-gray-45" />
                                )}
                            </Fragment>
                        );
                    })}
                </div>
            </Loading>
        </Overlay>
    );
}

export function FromAssets() {
    return (
        <DeepBookContextProvider>
            <FromAssetsComponent />
        </DeepBookContextProvider>
    );
}
