// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import Overlay from '_components/overlay';
import { useActiveAddress, useCoinsReFetchingConfig } from '_hooks';
import { TokenRow } from '_pages/home/tokens/TokensDetails';
import { useSuiClientQuery } from '@mysten/dapp-kit';
import { Fragment } from 'react';
import { useSearchParams } from 'react-router-dom';

function ToAsset({ coinType, onClick }: { coinType: string; onClick: (coinType: string) => void }) {
    const accountAddress = useActiveAddress();
    const [searchParams] = useSearchParams();
    const activeCoinType = searchParams.get('type');

    const { staleTime, refetchInterval } = useCoinsReFetchingConfig();

    const { data: coinBalance } = useSuiClientQuery(
        'getBalance',
        { coinType: coinType, owner: accountAddress! },
        { enabled: !!accountAddress, refetchInterval, staleTime },
    );

    if (!coinBalance || coinBalance.coinType === activeCoinType) {
        return null;
    }

    return (
        <TokenRow
            coinBalance={coinBalance}
            onClick={() => {
                onClick(coinType);
            }}
        />
    );
}

export function ToAssets({
    onClose,
    isOpen,
    onRowClick,
    recognizedCoins,
}: {
    onClose: () => void;
    isOpen: boolean;
    onRowClick: (coinType: string) => void;
    recognizedCoins: string[];
}) {
    return (
        <Overlay showModal={isOpen} title="Select a Coin" closeOverlay={onClose}>
            <div className="flex w-full flex-shrink-0 flex-col justify-start">
                {recognizedCoins.map((coinType, index) => (
                    <Fragment key={coinType}>
                        <ToAsset coinType={coinType} onClick={onRowClick} />
                        {index !== recognizedCoins.length - 1 && (
                            <div className="h-px w-full bg-gray-45" />
                        )}
                    </Fragment>
                ))}
            </div>
        </Overlay>
    );
}
