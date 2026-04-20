// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ErrorBoundary, MenuContent, Navigation, WalletSettingsButton } from '_components';
import cn from 'clsx';
import { createContext, type ReactNode, useState } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import { useLocation, Link } from 'react-router-dom';
import { useAppSelector, useActiveAccount } from '_hooks';
import { ExtensionViewType } from '../../redux/slices/app/appType';
import { Header } from '../header/Header';
import { Toaster } from '../toaster';
import { IotaLogoMark, Keystone, Ledger, Passkey } from '@iota/apps-ui-icons';
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
    const isPopup = extensionViewType === ExtensionViewType.Popup;
    const useSidebar = isFullScreen || isPopup;
    const [titlePortalContainer, setTitlePortalContainer] = useState<HTMLDivElement | null>(null);
    const isHomePage = window.location.hash === '#/tokens';
    const location = useLocation();
    // Key by the top-level route segment so only tab switches trigger the crossfade,
    // not every sub-page navigation within a section.
    const topLevelRoute = location.pathname.split('/')[1] ?? '';

    return (
        <div
            className={cn(
                'flex max-h-full w-full flex-1 flex-col flex-nowrap items-stretch overflow-hidden',
                useSidebar ? 'h-full rounded-xl' : 'justify-center',
            )}
        >
            {useSidebar ? (
                // Sidebar on the left for popup/fullscreen (only when bottom nav is enabled, i.e. not on dapp approval pages)
                <div className="flex w-full flex-1 flex-row overflow-hidden">
                    {bottomNavEnabled && <Navigation />}
                    <div
                        className={cn(
                            'flex flex-1 flex-col flex-nowrap overflow-hidden',
                            bottomNavEnabled ? 'px-md py-sm' : '',
                        )}
                    >
                        <div className="relative flex flex-grow flex-col flex-nowrap overflow-hidden">
                            <div id="overlay-portal-container" />
                            <div className="flex h-full flex-col overflow-hidden bg-iota-neutral-100 dark:bg-iota-neutral-6">
                                <main className="flex h-full w-full flex-col overflow-hidden">
                                    <AnimatePresence initial={false}>
                                        <motion.div
                                            key={topLevelRoute}
                                            initial={{ opacity: 0, y: 8 }}
                                            animate={{ opacity: 1, y: 0 }}
                                            transition={{
                                                duration: 0.22,
                                                ease: [0.16, 1, 0.3, 1],
                                            }}
                                            className="flex h-full w-full flex-col"
                                        >
                                            <PageMainLayoutContext.Provider
                                                value={titlePortalContainer}
                                            >
                                                <ErrorBoundary>{children}</ErrorBoundary>
                                            </PageMainLayoutContext.Provider>
                                        </motion.div>
                                    </AnimatePresence>
                                </main>
                                <Toaster bottomNavEnabled={bottomNavEnabled} />
                            </div>
                            {topNavMenuEnabled ? <MenuContent /> : null}
                        </div>
                    </div>
                </div>
            ) : (
                // Popup / SidePanel: original bottom-nav layout
                <>
                    {isHomePage ? (
                        <Header
                            leftContent={<LeftContent account={activeAccount} />}
                            middleContent={<div ref={setTitlePortalContainer} />}
                            rightContent={topNavMenuEnabled ? <WalletSettingsButton /> : undefined}
                        />
                    ) : null}
                    <div className="relative flex flex-grow flex-col flex-nowrap overflow-hidden">
                        <div id="overlay-portal-container" />
                        <div className="flex h-full flex-col overflow-hidden bg-iota-neutral-100 dark:bg-iota-neutral-6">
                            <main
                                className={cn('flex h-full w-full flex-col overflow-hidden', {
                                    'p-5': bottomNavEnabled && isHomePage,
                                })}
                            >
                                <AnimatePresence initial={false}>
                                    <motion.div
                                        key={topLevelRoute}
                                        initial={{ opacity: 0, y: 8 }}
                                        animate={{ opacity: 1, y: 0 }}
                                        transition={{
                                            duration: 0.22,
                                            ease: [0.16, 1, 0.3, 1],
                                        }}
                                        className="flex h-full w-full flex-col"
                                    >
                                        <PageMainLayoutContext.Provider
                                            value={titlePortalContainer}
                                        >
                                            <ErrorBoundary>{children}</ErrorBoundary>
                                        </PageMainLayoutContext.Provider>
                                    </motion.div>
                                </AnimatePresence>
                            </main>
                            <Toaster bottomNavEnabled={bottomNavEnabled} />
                        </div>
                        {topNavMenuEnabled ? <MenuContent /> : null}
                    </div>
                    {bottomNavEnabled ? <Navigation /> : null}
                </>
            )}
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
            className="flex flex-row items-center gap-sm p-xs no-underline"
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
