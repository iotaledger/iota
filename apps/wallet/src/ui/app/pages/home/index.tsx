// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Loading } from '_components';
import { useInitializedGuard, useSetGrowthbookAttributes } from '_hooks';
import { PageMainLayout } from '_src/ui/app/shared/page-main-layout/PageMainLayout';
import { Outlet } from 'react-router-dom';

interface HomePageProps {
    disableNavigation?: boolean;
}

export function HomePage({ disableNavigation }: HomePageProps) {
    const initChecking = useInitializedGuard(true);
    const guardChecking = initChecking;

    useSetGrowthbookAttributes();
    return (
        <Loading loading={guardChecking}>
            <PageMainLayout
                bottomNavEnabled={!disableNavigation}
                dappStatusEnabled={!disableNavigation}
                topNavMenuEnabled={!disableNavigation}
            >
                <Outlet />
            </PageMainLayout>
        </Loading>
    );
}

export * from './nfts';
export * from './assets';
export * from './tokens';
export * from './transactions';
export * from './transfer-coin';
export * from './nft-details';
export * from './kiosk-details';
export * from './nft-transfer';
export * from './receipt';
export * from './transfer-coin/CoinSelector';
export * from './apps';
