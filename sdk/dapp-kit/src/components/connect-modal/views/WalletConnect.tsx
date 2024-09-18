// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { WalletWithRequiredFeatures } from '@iota/wallet-standard';
import { useState } from 'react';
import { getWalletUniqueIdentifier } from '../../../utils/walletUtils.js';
import { useConnectWallet } from '../../../hooks/wallet/useConnectWallet.js';
import { WalletList } from '../wallet-list/WalletList.js';
import { ConnectionStatus } from '../footers/index.js';

interface WalletConnectListProps {
    wallets: WalletWithRequiredFeatures[];
    onOpenChange: (isOpen: boolean) => void;
}
export function WalletConnectList({ wallets, onOpenChange }: WalletConnectListProps) {
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
            <div className="flex flex-col items-center p-xs--rs">
                <WalletList
                    selectedWalletName={getWalletUniqueIdentifier(selectedWallet)}
                    onSelect={handleSelectWallet}
                    wallets={wallets}
                />
            </div>
            {selectedWallet && (
                <div className="flex w-full flex-col justify-center px-md--rs py-sm--rs">
                    <ConnectionStatus
                        selectedWallet={selectedWallet}
                        hadConnectionError={isError}
                        onRetryConnection={connectWallet}
                    />
                </div>
            )}
        </>
    );
}
