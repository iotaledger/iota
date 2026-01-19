// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ReactNode } from 'react';
import type { WalletWithRequiredFeatures } from '@iota/wallet-standard';

import { useCurrentAccount } from '../hooks/wallet/useCurrentAccount.js';
import { AccountDropdownMenu } from './AccountDropdownMenu.js';
import { ConnectModal } from './connect-modal/ConnectModal.js';
import { StyleMarker } from './styling/StyleMarker.js';
import { Button } from './ui/Button.js';

type ConnectButtonProps = {
    connectText?: ReactNode;
    size?: React.ComponentProps<typeof Button>['size'];
    iotaNamesEnabled?: boolean;
    onConnected?: (params: { wallet: WalletWithRequiredFeatures }) => void;
} & React.ComponentProps<typeof Button>;

export function ConnectButton({
    connectText = 'Connect Wallet',
    size,
    iotaNamesEnabled = true,
    onConnected,
    ...buttonProps
}: ConnectButtonProps) {
    const currentAccount = useCurrentAccount();
    return currentAccount ? (
        <AccountDropdownMenu
            currentAccount={currentAccount}
            size={size}
            iotaNamesEnabled={iotaNamesEnabled}
        />
    ) : (
        <ConnectModal
            onConnected={onConnected}
            trigger={
                <StyleMarker>
                    <Button {...buttonProps} size={size}>
                        {connectText}
                    </Button>
                </StyleMarker>
            }
        />
    );
}
