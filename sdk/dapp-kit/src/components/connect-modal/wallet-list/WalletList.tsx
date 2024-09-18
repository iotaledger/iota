// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { WalletWithRequiredFeatures } from '@iota/wallet-standard';
import { WalletListItem } from './WalletListItem.js';
import { getWalletUniqueIdentifier } from '../../../utils/walletUtils.js';

interface WalletListProps {
    selectedWalletName?: string;
    onSelect: (wallet: WalletWithRequiredFeatures) => void;
    wallets: WalletWithRequiredFeatures[];
}

export function WalletList({ selectedWalletName, onSelect, wallets }: WalletListProps) {
    return (
        <>
            {wallets.map((wallet) => (
                <WalletListItem
                    key={getWalletUniqueIdentifier(wallet)}
                    name={wallet.name}
                    icon={wallet.icon}
                    isSelected={getWalletUniqueIdentifier(wallet) === selectedWalletName}
                    onClick={() => onSelect(wallet)}
                />
            ))}
        </>
    );
}
