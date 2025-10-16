// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ConnectButton } from '@rainbow-me/rainbowkit';
import { useEffect } from 'react';
import { useAccount } from 'wagmi';
import { setWalletUserProperties, clearWalletUserProperties } from '../../../shared/analytics';

interface ConnectButtonL2Props {
    text?: string;
}

export function ConnectButtonL2({
    text = 'Connect L2 Wallet',
}: ConnectButtonL2Props): React.JSX.Element {
    const l2Account = useAccount();

    useEffect(() => {
        if (l2Account.isConnected && l2Account.address) {
            // Set wallet info as user properties (attached to all future events)
            setWalletUserProperties({
                l2WalletType: l2Account.connector?.name || 'unknown',
                l2ChainId: l2Account.chainId?.toString() || 'unknown',
            });
        } else {
            // Clear wallet info when disconnected
            clearWalletUserProperties('l2');
        }
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
