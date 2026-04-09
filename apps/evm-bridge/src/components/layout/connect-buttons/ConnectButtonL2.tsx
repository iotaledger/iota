// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ConnectButton } from '@rainbow-me/rainbowkit';
import { useEffect } from 'react';
import { useAccount } from 'wagmi';
import { ampli, setAmplitudeIdentity } from '../../../shared/analytics';

interface ConnectButtonL2Props {
    text?: string;
}

export function ConnectButtonL2({
    text = 'Connect L2 Wallet',
}: ConnectButtonL2Props): React.JSX.Element {
    const l2Account = useAccount();

    useEffect(() => {
        if (l2Account.isConnected && l2Account.address) {
            const walletType = l2Account.connector?.name || 'unknown';
            const chainId = l2Account.chainId?.toString() || 'unknown';
            setAmplitudeIdentity({ l2WalletType: walletType, l2ChainId: chainId });
            ampli.connectedL2Wallet({ walletType, chainId });
        } else {
            setAmplitudeIdentity({ l2WalletType: '', l2ChainId: '' });
        }
    }, [l2Account.isConnected, l2Account.address, l2Account.connector?.name, l2Account.chainId]);

    return (
        <div className="text-label-lg" data-testid="connect-l2-wallet" data-amp-mask>
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
