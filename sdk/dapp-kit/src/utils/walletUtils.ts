// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type {
    MinimallyRequiredFeatures,
    Wallet,
    WalletWithFeatures,
    WalletWithRequiredFeatures,
} from '@iota/wallet-standard';
import { getWallets, isWalletWithRequiredFeatureSet } from '@iota/wallet-standard';

export function getRegisteredWallets<AdditionalFeatures extends Wallet['features']>(
    preferredWallets: string[],
    walletFilter?: (wallet: WalletWithRequiredFeatures) => boolean,
) {
    const walletsApi = getWallets();
    const wallets = walletsApi.get();

    const iotaWallets = wallets.filter(
        (wallet): wallet is WalletWithFeatures<MinimallyRequiredFeatures & AdditionalFeatures> =>
            isWalletWithRequiredFeatureSet(wallet) && (!walletFilter || walletFilter(wallet)),
    );

    return [
        // Preferred wallets, in order:
        ...(preferredWallets
            .map((name) => iotaWallets.find((wallet) => wallet.name === name))
            .filter(Boolean) as WalletWithFeatures<
            MinimallyRequiredFeatures & AdditionalFeatures
        >[]),

        // Wallets in default order:
        ...iotaWallets.filter((wallet) => !preferredWallets.includes(wallet.name)),
    ];
}

export function getWalletUniqueIdentifier(wallet?: Wallet) {
    return wallet?.id ?? wallet?.name;
}
