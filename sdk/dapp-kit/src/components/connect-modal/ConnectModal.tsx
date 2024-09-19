// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Dialog, DialogContent, Divider, Header } from '@iota/apps-ui-kit';
import { useWallets } from '../../hooks/wallet/useWallets.js';
import { GetTheWalletView } from './views/GetTheWallet.js';
import { WalletConnectList } from './views/WalletConnect.js';
import { useState } from 'react';

interface ConnectModalProps {
    trigger?: React.ReactNode;
    open?: boolean;
    onOpenChange?: (isOpen: boolean) => void;
}

export function ConnectModal({ trigger, open: isModalOpen, onOpenChange }: ConnectModalProps) {
    const wallets = useWallets();
    const [isOpen, setIsOpen] = useState<boolean>(isModalOpen ?? false);

    function handleOpenChange(open: boolean) {
        setIsOpen(open);
        onOpenChange?.(open);
    }

    return (
        <Dialog open={isOpen ?? isModalOpen} onOpenChange={handleOpenChange}>
            <div onClick={() => handleOpenChange(!isOpen)}>{trigger}</div>
            <DialogContent showCloseOnOverlay>
                <Header title="Connect a Wallet" onClose={() => handleOpenChange(false)} />
                <Divider />
                {wallets?.length ? (
                    <WalletConnectList wallets={wallets} onOpenChange={handleOpenChange} />
                ) : (
                    <GetTheWalletView />
                )}
            </DialogContent>
        </Dialog>
    );
}
