// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Header, Bridge, Footer } from './components';
import { DirectionalArrowsSvg } from './components/svgs/DirectionalArrows';
import { FaucetButton } from './components/FaucetButton';
import { useEffect, useRef } from 'react';
import { useCurrentAccount, useCurrentWallet } from '@iota/dapp-kit';
import { useAccount } from 'wagmi';
import { ampli, initAmplitude } from './shared/analytics';

const TITLE = 'Seamlessly transfer funds between IOTA & IOTA EVM';

export default function App() {
    const l1Account = useCurrentAccount();
    const l1Wallet = useCurrentWallet();
    const l2Account = useAccount();

    const l1TrackedRef = useRef(false);
    const l2TrackedRef = useRef(false);

    useEffect(() => {
        initAmplitude();
    }, []);

    useEffect(() => {
        if (l1Wallet.isConnected && l1Account?.address && !l1TrackedRef.current) {
            ampli.connectedL1Wallet({
                walletType: l1Wallet.currentWallet?.name || 'unknown',
            });
            l1TrackedRef.current = true;
        }

        if (!l1Wallet.isConnected) {
            l1TrackedRef.current = false;
        }
    }, [l1Wallet.isConnected, l1Wallet.currentWallet?.name, l1Account?.address]);

    useEffect(() => {
        if (l2Account.isConnected && l2Account.address && !l2TrackedRef.current) {
            ampli.connectedL2Wallet({
                walletType: l2Account.connector?.name || 'unknown',
                chainId: l2Account.chainId?.toString() || 'unknown',
            });
            l2TrackedRef.current = true;
        }

        if (!l2Account.isConnected) {
            l2TrackedRef.current = false;
        }
    }, [l2Account.isConnected, l2Account.address, l2Account.connector?.name, l2Account.chainId]);

    return (
        <>
            <div className="relative overflow-x-hidden" id="app">
                <Header />

                <main className="flex flex-col min-h-screen container justify-center pt-24 flex-1">
                    <div className="flex flex-col items-center md:flex-row gap-lg h-full py-20">
                        <div className="flex-1 pb-2xl flex flex-col items-center md:items-start justify-center">
                            <div className="flex flex-col max-md:items-center max-md:text-center md:flex-col gap-10 pb-[72px] ">
                                <DirectionalArrowsSvg />
                                <h1 className="text-display-md text-balance text-iota-neutral-10 dark:text-iota-neutral-92 max-w-lg">
                                    {TITLE}
                                </h1>
                            </div>
                            <div>
                                <FaucetButton />
                            </div>
                        </div>

                        <div className="w-full max-w-[500px] md:w-2/5">
                            <Bridge />
                        </div>
                    </div>
                </main>
            </div>

            <Footer />
        </>
    );
}
