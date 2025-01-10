// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type PasswordRecoveryData } from '_src/shared/messaging/messages/payloads/methodPayload';
import { useMutation } from '@tanstack/react-query';

import { useForgotPasswordContext } from '../pages/accounts/forgot-password/ForgotPasswordPage';
import { useBackgroundClient } from './useBackgroundClient';

export function useRecoveryDataMutation() {
    const backgroundClient = useBackgroundClient();
    const { add } = useForgotPasswordContext();
    return useMutation({
        mutationKey: ['add recovery data'],
        mutationFn: async (data: PasswordRecoveryData) => {
            await backgroundClient.verifyPasswordRecoveryData({ data });
            add(data);
        },
    });
}
