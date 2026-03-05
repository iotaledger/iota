// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ErrorBoundary, MenuContent, Navigation, WalletSettingsButton } from '_components';
import cn from 'clsx';
import { createContext, type ReactNode, useState } from 'react';
import { useAppSelector, useActiveAccount } from '_hooks';
import { ExtensionViewType } from '../../redux/slices/app/appType';
import { Header } from '../header/Header';
import { Toaster } from '../toaster';
import { IotaLogoMark, Keystone, Ledger, Passkey } from '@iota/apps-ui-icons';
import { Link } from 'react-router-dom';
import { isLedgerAccountSerializedUI } from '_src/background/accounts/ledgerAccount';
import { type SerializedUIAccount } from '_src/background/accounts/account';
import { Badge, BadgeType } from '@iota/apps-ui-kit';
import { isLegacyAccount } from '_src/background/accounts/isLegacyAccount';
import { useGetDefaultIotaName } from '@iota/core';
import { formatAccountName } from '../../helpers';
import { isKeystoneAccountSerializedUI } from '_src/background/accounts/keystoneAccount';
import { isPasskeyAccountSerializedUI } from '_src/background/accounts/passkeyAccount';

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
}: PageMainLayoutProps) {
    const extensionViewType = useAppSelector((state) => state.app.extensionViewType);
    const activeAccount = useActiveAccount();
    const isFullScreen = extensionViewType === ExtensionViewType.FullScreen;
    const [titlePortalContainer, setTitlePortalContainer] = useState<HTMLDivElement | null>(null);
    const isHomePage = window.location.hash === '#/tokens';

    return (
        <div
            className={cn(
                'flex max-h-full w-full flex-1 flex-col flex-nowrap items-stretch justify-center overflow-hidden',
                isFullScreen ? 'rounded-xl' : '',
            )}
        >
            {isHomePage ? (
                <Header
                    leftContent={<LeftContent account={activeAccount} />}
                    middleContent={<div ref={setTitlePortalContainer} />}
                    rightContent={topNavMenuEnabled ? <WalletSettingsButton /> : undefined}
                />
            ) : null}
            <div className="relative flex flex-grow flex-col flex-nowrap overflow-hidden">
                <div className="flex flex-grow flex-col flex-nowrap overflow-y-auto overflow-x-hidden bg-iota-neutral-100 dark:bg-iota-neutral-6">
                    <main
                        className={cn('flex w-full flex-grow flex-col', {
                            'p-5': bottomNavEnabled && isHomePage,
                            'h-full': !isHomePage,
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

function LeftContent({ account }: { account: SerializedUIAccount | null }) {
    const { data: iotaName } = useGetDefaultIotaName(account?.address);
    const accountName = formatAccountName(account?.nickname, iotaName, account?.address);

    const isLedgerAccount = account && isLedgerAccountSerializedUI(account);
    const isKeystoneAccount = account && isKeystoneAccountSerializedUI(account);
    const isPasskeyAccount = account && isPasskeyAccountSerializedUI(account);

    return (
        <Link
            to="/accounts/manage"
            className="flex flex-row items-center gap-sm p-xs text-pink-200 no-underline"
            data-testid="accounts-manage"
        >
            <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-iota-primary-30 [&_svg]:h-5 [&_svg]:w-5 [&_svg]:text-white">
                {isLedgerAccount ? (
                    <Ledger />
                ) : isKeystoneAccount ? (
                    <Keystone />
                ) : isPasskeyAccount ? (
                    <Passkey />
                ) : (
                    <IotaLogoMark />
                )}
            </div>
            <div className="flex flex-col items-start">
                <span
                    className="text-title-sm text-iota-neutral-10 dark:text-iota-neutral-92"
                    data-amp-mask
                >
                    {accountName}
                </span>
            </div>
            {isLegacyAccount(account) && <Badge type={BadgeType.Neutral} label="Legacy" />}
        </Link>
    );
}
