// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { requestIotaFromFaucet } from '@iota/iota-sdk/faucet';
import { useIsMutating, useMutation, type UseMutationOptions } from '@tanstack/react-query';
import { useActiveAccount } from '_hooks';

type UseFaucetMutationOptions = Pick<UseMutationOptions, 'onError'> & {
    host?: string;
    address?: string;
};

export function useFaucetMutation(options?: UseFaucetMutationOptions) {
    const activeAccount = useActiveAccount();
    const activeAddress = activeAccount?.address || null;
    const addressToTopUp = options?.address || activeAddress;
    const mutationKey = ['faucet-request-tokens', activeAddress];
    const mutation = useMutation({
        mutationKey,
        mutationFn: async () => {
            if (!addressToTopUp) {
                throw new Error('Failed, wallet address not found.');
            }
            if (!options?.host) {
                throw new Error('Failed, faucet host not found.');
            }
            return requestIotaFromFaucet({
                recipient: addressToTopUp,
                host: options.host,
            });
        },
        ...options,
    });
    return {
        ...mutation,
        /** If the currently-configured endpoint supports faucet and the active account is unlocked */
        enabled: !!options?.host && !!activeAccount && !activeAccount.isLocked,
        /**
         * is any faucet request in progress across different instances of the mutation
         */
        isMutating: useIsMutating({ mutationKey }) > 0,
    };
}
