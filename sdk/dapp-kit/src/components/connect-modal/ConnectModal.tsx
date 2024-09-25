// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as Dialog from '@radix-ui/react-dialog';
import { useState } from 'react';
import type { ReactNode } from 'react';
import { CloseIcon } from '../icons/CloseIcon.js';
import { StyleMarker } from '../styling/StyleMarker.js';
import { Heading } from '../ui/Heading.js';
import { IconButton } from '../ui/IconButton.js';
import * as styles from './ConnectModal.css.js';
import { useWallets } from '../../hooks/wallet/useWallets.js';
import { WalletConnectListView } from './views/WalletConnect.js';
import { GetTheWalletView } from './views/GetTheWallet.js';

type ControlledModalProps = {
    /** The controlled open state of the dialog. */
    open: boolean;

    /** Event handler called when the open state of the dialog changes. */
    onOpenChange: (open: boolean) => void;

    defaultOpen?: never;
};

type UncontrolledModalProps = {
    open?: never;

    onOpenChange?: never;

    /** The open state of the dialog when it is initially rendered. Use when you do not need to control its open state. */
    defaultOpen?: boolean;
};

type ConnectModalProps = {
    /** The trigger button that opens the dialog. */
    trigger: NonNullable<ReactNode>;
} & (ControlledModalProps | UncontrolledModalProps);

export function ConnectModal({ trigger, open, defaultOpen, onOpenChange }: ConnectModalProps) {
    const wallets = useWallets();
    const [isModalOpen, setModalOpen] = useState(open ?? defaultOpen);

    const handleOpenChange = (open: boolean) => {
        setModalOpen(open);
        onOpenChange?.(open);
    };

    return (
        <Dialog.Root open={open ?? isModalOpen} onOpenChange={handleOpenChange}>
            <Dialog.Trigger asChild>{trigger}</Dialog.Trigger>
            <Dialog.Portal>
                <StyleMarker>
                    <Dialog.Overlay className={styles.overlay}>
                        <Dialog.Content className={styles.content} aria-describedby={undefined}>
                            <div className={styles.walletListContent}>
                                <Dialog.Title className={styles.title} asChild>
                                    <Heading as="h2">Connect a Wallet</Heading>
                                </Dialog.Title>
                                <div className={styles.separator} />
                                {wallets?.length ? (
                                    <WalletConnectListView
                                        wallets={wallets}
                                        onOpenChange={handleOpenChange}
                                    />
                                ) : (
                                    <GetTheWalletView />
                                )}
                            </div>
                            <Dialog.Close className={styles.closeButtonContainer} asChild>
                                <IconButton type="button" aria-label="Close">
                                    <CloseIcon />
                                </IconButton>
                            </Dialog.Close>
                        </Dialog.Content>
                    </Dialog.Overlay>
                </StyleMarker>
            </Dialog.Portal>
        </Dialog.Root>
    );
}
