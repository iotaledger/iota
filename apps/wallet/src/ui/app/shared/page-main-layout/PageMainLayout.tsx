// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ErrorBoundary, MenuContent, Navigation, WalletSettingsButton } from '_components';
import cn from 'clsx';
import { createContext, useState, type ReactNode } from 'react';

import { useAppSelector } from '../../hooks';
import { AppType } from '../../redux/slices/app/AppType';
import DappStatus from '../dapp-status';
import { Header } from '../header/Header';
import { Toaster } from '../toaster';
import { IotaLogoMark, Ledger } from '@iota/ui-icons';
import { useActiveAccount } from '../../hooks/useActiveAccount';
import { Link } from 'react-router-dom';
import { formatAddress } from '@iota/iota-sdk/utils';
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
    const isLedgerAccount = activeAccount && isLedgerAccountSerializedUI(activeAccount);

    return (
        <div
            className={cn(
                'flex max-h-full w-full flex-1 flex-col flex-nowrap items-stretch justify-center overflow-hidden',
                isFullScreen ? 'rounded-xl' : '',
            )}
        >
            <Header
                network={network}
                leftContent={
                    <LeftContent
                        account={activeAccount?.address}
                        isLedgerAccount={isLedgerAccount}
                        isLocked={activeAccount?.isLocked}
                    />
                }
                middleContent={
                    dappStatusEnabled ? <DappStatus /> : <div ref={setTitlePortalContainer} />
                }
                rightContent={topNavMenuEnabled ? <WalletSettingsButton /> : undefined}
            />
            <div className="relative flex flex-grow flex-col flex-nowrap overflow-hidden">
                <div className="flex flex-grow flex-col flex-nowrap overflow-y-auto overflow-x-hidden bg-neutral-100">
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

function LeftContent({
    account,
    isLedgerAccount,
    isLocked,
}: {
    account: string | undefined;
    isLedgerAccount: boolean | null;
    isLocked?: boolean;
}) {
    const backgroundColor = isLocked ? 'bg-neutral-90' : 'bg-primary-30';
    return (
        <div className="flex flex-row items-center gap-sm p-xs">
            <Link to="/" className="text-gray-90 no-underline">
                <div
                    className={cn(
                        'rounded-full p-1 text-neutral-100 [&_svg]:h-5 [&_svg]:w-5',
                        backgroundColor,
                    )}
                >
                    {isLedgerAccount ? <Ledger /> : <IotaLogoMark />}
                </div>
            </Link>
            <Link to="/accounts/manage" className="text-gray-90 no-underline">
                <span className="text-title-sm text-neutral-10">
                    {formatAddress(account || '')}
                </span>
            </Link>
        </div>
    );
}
