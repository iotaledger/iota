// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung 
// SPDX-License-Identifier: Apache-2.0

import { ActiveCoinsCard } from '_components/active-coins-card';
import Overlay from '_components/overlay';
import { useUnlockedGuard } from '_src/ui/app/hooks/useUnlockedGuard';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';
import { useNavigate, useSearchParams } from 'react-router-dom';

function CoinsSelectorPage() {
    const [searchParams] = useSearchParams();
    const coinType = searchParams.get('type') || IOTA_TYPE_ARG;
    const navigate = useNavigate();

    if (useUnlockedGuard()) {
        return null;
    }

    return (
        <Overlay
            showModal={true}
            title="Select Coin"
            closeOverlay={() =>
                navigate(
                    `/send?${new URLSearchParams({
                        type: coinType,
                    }).toString()}`,
                )
            }
        >
            <ActiveCoinsCard activeCoinType={coinType} showActiveCoin={false} />
        </Overlay>
    );
}

export default CoinsSelectorPage;
