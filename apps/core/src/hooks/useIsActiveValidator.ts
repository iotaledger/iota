// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import { useCallback } from 'react';

export function useIsActiveValidator() {
    const { data: systemState } = useIotaClientQuery('getLatestIotaSystemState');

    const isActiveValidator = useCallback(
        (address: string) =>
            systemState?.activeValidators.some((member) => member.iotaAddress === address),
        [systemState?.activeValidators],
    );

    return { isActiveValidator };
}
