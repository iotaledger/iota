// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ErrorBoundary } from '_components/error-boundary';
import { MenuContent } from '_components/menu';
import { Navigation } from '_components/navigation';
import cn from 'clsx';
import { createContext, useState, type ReactNode } from 'react';

import { WalletSettingsButton } from '_components/menu/button/WalletSettingsButton';
import { useAppSelector } from '../../hooks';
import { AppType } from '../../redux/slices/app/AppType';
import DappStatus from '../dapp-status';
import { Header } from '../header/Header';
import { Toaster } from '../toaster';

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
    const isFullScreen = appType === AppType.Fullscreen;
    const [titlePortalContainer, setTitlePortalContainer] = useState<HTMLDivElement | null>(null);

    return (
        <div
            className={cn(
                'flex max-h-full w-full flex-1 flex-col flex-nowrap items-stretch justify-center overflow-hidden bg-gradients-graph-cards',
                isFullScreen ? 'rounded-xl' : '',
            )}
        >
            <Header
                network={network}
                middleContent={
                    dappStatusEnabled ? <DappStatus /> : <div ref={setTitlePortalContainer} />
                }
                rightContent={topNavMenuEnabled ? <WalletSettingsButton /> : undefined}
            />
            <div className="relative flex flex-grow flex-col flex-nowrap overflow-hidden rounded-t-xl shadow-wallet-content">
                <div className="flex flex-grow flex-col flex-nowrap overflow-y-auto overflow-x-hidden rounded-t-xl bg-white">
                    <main
                        className={cn('flex w-full flex-grow flex-col', {
                            'p-5': bottomNavEnabled,
                        })}
                    >
                        <PageMainLayoutContext.Provider value={titlePortalContainer}>
                            <ErrorBoundary>{children}</ErrorBoundary>
                        </PageMainLayoutContext.Provider>
                    </main>
                    {bottomNavEnabled ? <Navigation /> : null}
                    <Toaster bottomNavEnabled={bottomNavEnabled} />
                </div>
                {topNavMenuEnabled ? <MenuContent /> : null}
            </div>
        </div>
    );
}
