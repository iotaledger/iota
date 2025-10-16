// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ConnectButton } from '@iota/dapp-kit';
import { useEffect, useRef } from 'react';
import { useCurrentAccount, useCurrentWallet } from '@iota/dapp-kit';
import { ampli } from '../../../shared/analytics';

interface ConnectButtonL1Props {
    connectText?: string;
    className?: string;
    size?: React.ComponentProps<typeof ConnectButton>['size'];
    iotaNamesEnabled?: boolean;
}

export function ConnectButtonL1({
    connectText = 'Connect L1 Wallet',
    className,
    size,
    iotaNamesEnabled = true,
}: ConnectButtonL1Props) {
    const l1Account = useCurrentAccount();
    const l1Wallet = useCurrentWallet();
    const prevConnectedRef = useRef(false);

    useEffect(() => {
        const isConnected = l1Wallet.isConnected && !!l1Account?.address;
        const wasConnected = prevConnectedRef.current;

        // Only track when transitioning from disconnected to connected (user action)
        if (isConnected && !wasConnected) {
            const storageKey = `l1_wallet_tracked_${l1Account.address}`;
            const hasTrackedInSession = sessionStorage.getItem(storageKey);

            if (!hasTrackedInSession) {
                ampli.connectedL1Wallet({
                    walletType: l1Wallet.currentWallet?.name || 'unknown',
                });
                sessionStorage.setItem(storageKey, 'true');
            }
        }

        // Clear session storage when disconnected
        if (!isConnected && wasConnected && l1Account?.address) {
            const storageKey = `l1_wallet_tracked_${l1Account.address}`;
            sessionStorage.removeItem(storageKey);
        }

        prevConnectedRef.current = isConnected;
    }, [l1Wallet.isConnected, l1Wallet.currentWallet?.name, l1Account?.address]);

    return (
        <ConnectButton
            data-testid="connect-l1-wallet"
            className={className}
            connectText={connectText}
            size={size}
            iotaNamesEnabled={iotaNamesEnabled}
        />
    );
}
