// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { ConnectButton, useCurrentWallet, useAutoConnectWallet } from '@iota/dapp-kit';
import { redirect } from 'next/navigation';
import { IotaLogoWeb } from '@iota/apps-ui-icons';
import { HOMEPAGE_ROUTE } from '@/lib/constants/routes.constants';
import { Theme, ThemeSwitcher, useTheme } from '@iota/core';
import { LoadingIndicator } from '@iota/apps-ui-kit';

function HomeDashboardPage(): JSX.Element {
    const { theme } = useTheme();
    const { isConnected } = useCurrentWallet();
    const autoConnect = useAutoConnectWallet();

    if (isConnected) {
        redirect(HOMEPAGE_ROUTE.path);
    }

    const CURRENT_YEAR = new Date().getFullYear();
    const videoSrc =
        theme === Theme.Dark
            ? 'https://files.iota.org/media/tooling/wallet-dashboard-welcome-dark.mp4'
            : 'https://files.iota.org/media/tooling/wallet-dashboard-welcome-light.mp4';

    return (
        <main className="flex h-screen">
            {autoConnect === 'idle' ? (
                <div className="flex w-full justify-center">
                    <LoadingIndicator size="w-16 h-16" />
                </div>
            ) : (
                <>
                    <div className="relative hidden sm:flex md:w-1/3">
                        <video
                            key={theme}
                            src={videoSrc}
                            autoPlay
                            muted
                            loop
                            className="absolute right-0 top-0 h-full w-full min-w-fit object-cover"
                            disableRemotePlayback
                        ></video>
                    </div>
                    <div className="relative flex h-full w-full flex-col items-center justify-between p-md sm:p-2xl">
                        <div className="absolute right-2 top-2 sm:right-8 sm:top-8">
                            <ThemeSwitcher />
                        </div>
                        <IotaLogoWeb width={130} height={32} />
                        <div className="flex max-w-sm flex-col items-center gap-8 text-center">
                            <div className="flex flex-col items-center gap-4">
                                <span className="text-headline-sm text-neutral-40">Welcome to</span>
                                <h1 className="text-display-lg text-neutral-10 dark:text-neutral-100">
                                    IOTA Wallet Dashboard
                                </h1>
                                <span className="text-title-lg text-neutral-40">
                                    Connecting you to the decentralized web and IOTA network
                                </span>
                            </div>
                            <div className="[&_button]:!bg-neutral-90 [&_button]:dark:!bg-neutral-20">
                                <ConnectButton connectText="Connect" />
                            </div>
                        </div>
                        <div className="text-center text-body-lg text-neutral-60">
                            &copy; IOTA Foundation {CURRENT_YEAR}
                            <br />
                            {process.env.NEXT_PUBLIC_DASHBOARD_REV}
                        </div>
                    </div>
                </>
            )}
        </main>
    );
}

export default HomeDashboardPage;
