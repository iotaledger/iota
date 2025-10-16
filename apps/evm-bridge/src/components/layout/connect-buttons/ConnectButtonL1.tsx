// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ConnectButton } from '@iota/dapp-kit';
import { useEffect } from 'react';
import { useCurrentAccount, useCurrentWallet } from '@iota/dapp-kit';
import { setWalletUserProperties, clearWalletUserProperties } from '../../../shared/analytics';

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

    useEffect(() => {
        if (l1Wallet.isConnected && l1Account?.address) {
            // Set wallet info as user properties (attached to all future events)
            setWalletUserProperties({
                l1WalletType: l1Wallet.currentWallet?.name || 'unknown',
            });
        } else {
            // Clear wallet info when disconnected
            clearWalletUserProperties('l1');
        }
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
