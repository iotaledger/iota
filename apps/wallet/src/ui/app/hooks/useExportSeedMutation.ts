// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type MethodPayload } from '_src/shared/messaging/messages/payloads/methodPayload';
import { useMutation } from '@tanstack/react-query';

import { useBackgroundClient } from './useBackgroundClient';

export function useExportSeedMutation() {
    const backgroundClient = useBackgroundClient();
    return useMutation({
        mutationKey: ['export-seed'],
        mutationFn: async (args: MethodPayload<'getAccountSourceSeed'>['args']) =>
            (await backgroundClient.getAccountSourceSeed(args)).seed,
    });
}
