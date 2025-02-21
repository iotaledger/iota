// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { WalletWithRequiredFeatures } from '@iota/wallet-standard';

import { useWallets } from '../../../hooks/wallet/useWallets.js';
import { getWalletUniqueIdentifier } from '../../../utils/walletUtils.js';
import { IotaIcon } from '../../icons/IotaIcon.js';
import * as styles from './WalletList.css.js';
import { WalletListItem } from './WalletListItem.js';

type WalletListProps = {
    selectedWalletName?: string;
    onPlaceholderClick: () => void;
    onSelect: (wallet: WalletWithRequiredFeatures) => void;
};

export function WalletList({ selectedWalletName, onPlaceholderClick, onSelect }: WalletListProps) {
    const wallets = useWallets();
    return (
        <ul className={styles.container}>
            {wallets.length > 0 ? (
                wallets.map((wallet) => (
                    <WalletListItem
                        key={getWalletUniqueIdentifier(wallet)}
                        name={wallet.name}
                        icon={wallet.icon}
                        isSelected={getWalletUniqueIdentifier(wallet) === selectedWalletName}
                        onClick={() => onSelect(wallet)}
                    />
                ))
            ) : (
                <WalletListItem
                    name="IOTA Wallet"
                    icon={<IotaIcon className={styles.icon} />}
                    onClick={onPlaceholderClick}
                    isSelected
                />
            )}
        </ul>
    );
}
