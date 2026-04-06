// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ConnectButton } from '@iota/dapp-kit';
import { useEffect } from 'react';
import { useCurrentAccount, useCurrentWallet } from '@iota/dapp-kit';
import { ampli, setAmplitudeIdentity } from '../../../shared/analytics';

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
            // TODO this event calls 2 times on page load, because we use component twice. Fix it later.
            const walletType = l1Wallet.currentWallet?.name || 'unknown';
            setAmplitudeIdentity({ l1WalletType: walletType });
            ampli.connectedL1Wallet({ walletType });
        } else {
            setAmplitudeIdentity({ l1WalletType: '' });
        }
    }, [l1Wallet.isConnected, l1Wallet.currentWallet?.name, l1Account?.address]);

    return (
        <div data-amp-mask>
            <ConnectButton
                data-testid="connect-l1-wallet"
                className={className}
                connectText={connectText}
                size={size}
                iotaNamesEnabled={iotaNamesEnabled}
            />
        </div>
    );
}
