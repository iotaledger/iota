// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount, useCurrentWallet } from '@iota/dapp-kit';
import { requestIotaFromFaucetV0 } from '@iota/iota-sdk/faucet';
import { useNetworkVariables } from '../config/l1config';
import { Button } from '@iota/apps-ui-kit';
import { useMutation } from '@tanstack/react-query';
import toast from 'react-hot-toast';

export function FaucetButton() {
    const { isConnected } = useCurrentWallet();
    const currentAccount = useCurrentAccount();
    const variables = useNetworkVariables();

    const recipient = currentAccount?.address;
    const isFaucetEnabled = !!variables.faucet;

    const { mutateAsync: requestFaucet, isPending } = useMutation({
        mutationKey: ['faucet-funds', recipient],
        async mutationFn() {
            toast('Requesting funds from faucet...');
            if (recipient && isFaucetEnabled) {
                await requestIotaFromFaucetV0({
                    host: variables.faucet,
                    recipient,
                });
            }
        },
        onSuccess() {
            toast.success('Funds successfully sent.');
        },
        onError() {
            toast.error('Something went wrong while requesting funds.');
        },
    });

    if (!isFaucetEnabled) {
        return null;
    }

    return (
        <Button
            text="Request funds"
            onClick={() => requestFaucet()}
            disabled={!isConnected || isPending}
            testId="request-l1-funds-button"
        />
    );
}
