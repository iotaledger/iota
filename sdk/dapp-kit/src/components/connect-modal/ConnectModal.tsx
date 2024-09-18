// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Dialog, DialogContent, Divider, Header } from '@iota/apps-ui-kit';
import { useWallets } from '../../hooks/wallet/useWallets.js';
import { GetTheWalletView } from './views/GetTheWallet.js';
import { WalletConnectList } from './views/WalletConnect.js';

interface ConnectModalProps {
    isModalOpen?: boolean;
    onOpenChange: (isOpen: boolean) => void;
}

export function ConnectModal({ isModalOpen, onOpenChange }: ConnectModalProps) {
    const wallets = useWallets();

    return (
        <Dialog open={isModalOpen} onOpenChange={onOpenChange}>
            <DialogContent showCloseOnOverlay>
                <Header title="Connect a Wallet" onClose={() => onOpenChange(false)} />
                <Divider />
                {wallets?.length ? (
                    <WalletConnectList wallets={wallets} onOpenChange={onOpenChange} />
                ) : (
                    <GetTheWalletView />
                )}
            </DialogContent>
        </Dialog>
    );
}
