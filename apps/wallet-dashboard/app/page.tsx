// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { ConnectButton, useCurrentAccount, useCurrentWallet } from '@iota/dapp-kit';
import { useEffect } from 'react';
import { useRouter } from 'next/navigation';
import { IotaLogoWeb } from '@iota/ui-icons';

function HomeDashboardPage(): JSX.Element {
    const { connectionStatus } = useCurrentWallet();
    const account = useCurrentAccount();
    const router = useRouter();

    const CURRENT_YEAR = new Date().getFullYear();

    useEffect(() => {
        if (connectionStatus === 'connected' && account) {
            router.push('/dashboard/home');
        }
    }, [connectionStatus, account, router]);

    return (
        <main className="flex h-screen">
            <div className="hidden sm:flex md:w-1/4">
                <video
                    autoPlay
                    muted
                    loop
                    className="h-full w-full object-cover"
                    disableRemotePlayback
                >
                    <source
                        src="https://files.iota.org/media/tooling/wallet-dashboard-welcome.mp4"
                        type="video/mp4"
                    />
                </video>
            </div>
            <div className="flex h-full w-full flex-col items-center justify-between p-md sm:p-2xl">
                <IotaLogoWeb width={130} height={32} />
                <div className="flex max-w-sm flex-col items-center gap-8 text-center">
                    <div className="flex flex-col items-center gap-4">
                        <span className="text-headline-sm text-neutral-40">Welcome to</span>
                        <h1 className="text-display-lg text-neutral-10">IOTA Wallet</h1>
                        <span className="text-title-lg text-neutral-40">
                            Connecting you to the decentralized web and IOTA network
                        </span>
                    </div>
                    <div className="[&_button]:!bg-neutral-90">
                        <ConnectButton connectText="Connect" />
                    </div>
                </div>
                <div className="text-body-lg text-neutral-60">
                    &copy; IOTA Foundation {CURRENT_YEAR}
                </div>
            </div>
        </main>
    );
}

export default HomeDashboardPage;
