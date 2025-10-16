// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ConnectButton } from '@rainbow-me/rainbowkit';
import { useEffect, useRef } from 'react';
import { useAccount } from 'wagmi';
import { ampli } from '../../../shared/analytics';

interface ConnectButtonL2Props {
    text?: string;
}

export function ConnectButtonL2({
    text = 'Connect L2 Wallet',
}: ConnectButtonL2Props): React.JSX.Element {
    const l2Account = useAccount();
    const prevConnectedRef = useRef(false);

    useEffect(() => {
        const isConnected = l2Account.isConnected && !!l2Account.address;
        const wasConnected = prevConnectedRef.current;

        // Only track when transitioning from disconnected to connected (user action)
        if (isConnected && !wasConnected) {
            const storageKey = `l2_wallet_tracked_${l2Account.address}`;
            const hasTrackedInSession = sessionStorage.getItem(storageKey);

            if (!hasTrackedInSession) {
                ampli.connectedL2Wallet({
                    walletType: l2Account.connector?.name || 'unknown',
                    chainId: l2Account.chainId?.toString() || 'unknown',
                });
                sessionStorage.setItem(storageKey, 'true');
            }
        }

        // Clear session storage when disconnected
        if (!isConnected && wasConnected && l2Account.address) {
            const storageKey = `l2_wallet_tracked_${l2Account.address}`;
            sessionStorage.removeItem(storageKey);
        }

        prevConnectedRef.current = isConnected;
    }, [l2Account.isConnected, l2Account.address, l2Account.connector?.name, l2Account.chainId]);

    return (
        <div className="text-label-lg" data-testid="connect-l2-wallet">
            <ConnectButton
                label={text}
                accountStatus={{
                    smallScreen: 'full',
                    largeScreen: 'full',
                }}
                showBalance={{
                    smallScreen: true,
                    largeScreen: true,
                }}
            />
        </div>
    );
}
