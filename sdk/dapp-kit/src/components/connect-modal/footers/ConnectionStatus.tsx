// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { WalletWithRequiredFeatures } from '@iota/wallet-standard';

import { Exclamation } from '@iota/ui-icons';
import { Button } from '@iota/apps-ui-kit';

interface ConnectionStatusProps {
    selectedWallet: WalletWithRequiredFeatures;
    hadConnectionError: boolean;
    onRetryConnection: (selectedWallet: WalletWithRequiredFeatures) => void;
}

export function ConnectionStatus({
    selectedWallet,
    hadConnectionError,
    onRetryConnection,
}: ConnectionStatusProps) {
    return (
        <>
            {hadConnectionError ? (
                <div className="flex w-full flex-row items-center justify-between gap-sm">
                    <p className="flex flex-row items-center gap-xxs text-body-md text-error-40 dark:text-error-60">
                        <Exclamation className="h-4 w-4" /> Connection failed
                    </p>
                    <Button
                        text="Retry Connection"
                        onClick={() => onRetryConnection(selectedWallet)}
                    />
                </div>
            ) : (
                <div className="flex flex-col items-start gap-y-xxxs">
                    <p className="text-body-lg text-neutral-10 dark:text-neutral-92">
                        Opening {selectedWallet.name}
                    </p>
                    <p className="dark:text-neutral-60 text-body-md text-neutral-40">
                        Confirm connection in the wallet...
                    </p>
                </div>
            )}
        </>
    );
}
