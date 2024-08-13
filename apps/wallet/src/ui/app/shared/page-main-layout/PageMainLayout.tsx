// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ErrorBoundary } from '_components/error-boundary';
import { MenuContent } from '_components/menu';
import { Navigation } from '_components/navigation';
import cn from 'clsx';
import { createContext, useState, type ReactNode } from 'react';
import { useAppSelector } from '../../hooks';
import { AppType } from '../../redux/slices/app/AppType';
import DappStatus from '../dapp-status';
import { Header } from '../header/Header';
import { Toaster } from '../toaster';
import { IotaLogoMark } from '@iota/ui-icons';
import { WalletSettingsButton } from '../../components/menu/button/WalletSettingsButton';
import { useActiveAccount } from '../../hooks/useActiveAccount';
import { Link } from 'react-router-dom';
import { formatAddress } from '@iota/iota-sdk/utils';
import { useAccountSources } from '../../hooks/useAccountSources';
import { isLedgerAccountSerializedUI } from '_src/background/accounts/LedgerAccount';

export const PageMainLayoutContext = createContext<HTMLDivElement | null>(null);

export interface PageMainLayoutProps {
    children: ReactNode | ReactNode[];
    bottomNavEnabled?: boolean;
    topNavMenuEnabled?: boolean;
    dappStatusEnabled?: boolean;
}

export function PageMainLayout({
    children,
    bottomNavEnabled = false,
    topNavMenuEnabled = false,
    dappStatusEnabled = false,
}: PageMainLayoutProps) {
    const network = useAppSelector(({ app: { network } }) => network);
    const appType = useAppSelector((state) => state.app.appType);
    const activeAccount = useActiveAccount();
    const isFullScreen = appType === AppType.Fullscreen;
    const [titlePortalContainer, setTitlePortalContainer] = useState<HTMLDivElement | null>(null);
    const { data: accountSources } = useAccountSources();
    const isLedgerAccount = activeAccount && isLedgerAccountSerializedUI(activeAccount);
    console.log('accountSources', accountSources);
    console.log('isLedeger', isLedgerAccount);
    return (
        <div
            className={cn(
                'flex max-h-full w-full flex-1 flex-col flex-nowrap items-stretch justify-center overflow-hidden',
                isFullScreen ? 'rounded-xl' : '',
            )}
        >
            <Header
                network={network}
                leftContent={<LeftContent account={activeAccount?.address} />}
                middleContent={
                    dappStatusEnabled ? <DappStatus /> : <div ref={setTitlePortalContainer} />
                }
                rightContent={topNavMenuEnabled ? <WalletSettingsButton /> : undefined}
            />
            <div className="relative flex flex-grow flex-col flex-nowrap overflow-hidden shadow-wallet-content">
                <div className="flex flex-grow flex-col flex-nowrap overflow-y-auto overflow-x-hidden bg-white">
                    <main
                        className={cn('flex w-full flex-grow flex-col', {
                            'p-5': bottomNavEnabled,
                        })}
                    >
                        <PageMainLayoutContext.Provider value={titlePortalContainer}>
                            <ErrorBoundary>{children}</ErrorBoundary>
                        </PageMainLayoutContext.Provider>
                    </main>
                    <Toaster bottomNavEnabled={bottomNavEnabled} />
                </div>
                {topNavMenuEnabled ? <MenuContent /> : null}
            </div>
            {bottomNavEnabled ? <Navigation /> : null}
        </div>
    );
}

function LeftContent({ account }: { account: string | undefined }) {
    return (
        <div className="flex flex-row items-center gap-sm">
            <Link to="/" className="text-gray-90 no-underline">
                <IotaLogoMark className="h-6 w-6" />
            </Link>
            <span className="text-title-sm text-neutral-10">{formatAddress(account || '')}</span>
        </div>
    );
}
