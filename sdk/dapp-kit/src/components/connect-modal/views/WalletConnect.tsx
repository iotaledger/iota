// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { WalletWithRequiredFeatures } from '@iota/wallet-standard';
import { useState } from 'react';
import { getWalletUniqueIdentifier } from '../../../utils/walletUtils.js';
import { useConnectWallet } from '../../../hooks/wallet/useConnectWallet.js';
import { WalletList } from '../wallet-list/WalletList.js';
import * as styles from './WalletConnect.css.js';
import { Button } from '../../ui/Button.js';

interface WalletConnectListViewProps {
    wallets: WalletWithRequiredFeatures[];
    onOpenChange: (isOpen: boolean) => void;
}
export function WalletConnectListView({ wallets, onOpenChange }: WalletConnectListViewProps) {
    const [selectedWallet, setSelectedWallet] = useState<WalletWithRequiredFeatures>();
    const { mutate, isError } = useConnectWallet();
    function handleSelectWallet(wallet: WalletWithRequiredFeatures) {
        if (getWalletUniqueIdentifier(selectedWallet) !== getWalletUniqueIdentifier(wallet)) {
            setSelectedWallet(wallet);
            connectWallet(wallet);
        }
    }
    function resetSelection() {
        setSelectedWallet(undefined);
    }

    function handleOpenChange(open: boolean) {
        if (!open) {
            resetSelection();
        }
        onOpenChange(open);
    }

    function connectWallet(wallet: WalletWithRequiredFeatures) {
        mutate(
            { wallet },
            {
                onSuccess: () => handleOpenChange(false),
            },
        );
    }
    return (
        <>
            <div className={styles.walletConnectContainer}>
                <WalletList
                    selectedWalletName={getWalletUniqueIdentifier(selectedWallet)}
                    onSelect={handleSelectWallet}
                    wallets={wallets}
                />
            </div>
            {selectedWallet && (
                <div className={styles.walletConnectFooter}>
                    {isError ? (
                        <div className={styles.errorContainer}>
                            <p className={styles.errorMessage}>Connection failed</p>
                            <Button
                                size="md"
                                variant="outline"
                                onClick={() => connectWallet(selectedWallet)}
                            >
                                Retry
                            </Button>
                        </div>
                    ) : (
                        <div className={styles.successContainer}>
                            <p className={styles.openingText}>Opening {selectedWallet.name}...</p>
                            <div className={styles.separator} />
                            <p className={styles.confirmText}>
                                Please confirm the connection in your wallet
                            </p>
                        </div>
                    )}
                </div>
            )}
        </>
    );
}
