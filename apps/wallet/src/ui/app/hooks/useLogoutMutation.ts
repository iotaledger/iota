// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMutation } from '@tanstack/react-query';

import { ampli } from '_src/shared/analytics/ampli';
import { queryClient, persister } from '../helpers';
import { useBackgroundClient } from './useBackgroundClient';

export function useLogoutMutation() {
    const backgroundClient = useBackgroundClient();

    return useMutation({
        mutationKey: ['logout', 'clear wallet'],
        mutationFn: async () => {
            await ampli.resetWallet().promise;
            await ampli.flush().promise;
            ampli.client.reset();
            await queryClient.cancelQueries();
            queryClient.clear();
            await persister.removeClient();
            await backgroundClient.clearWallet();
        },
    });
}
