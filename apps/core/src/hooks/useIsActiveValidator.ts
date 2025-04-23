// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback } from 'react';
import { useGetLatestIotaSystemState } from './useGetLatestIotaSystemState';

export function useIsActiveValidator() {
    const { data: systemState } = useGetLatestIotaSystemState();

    const isActiveValidator = useCallback(
        (address: string) =>
            systemState?.activeValidators.some((member) => member.iotaAddress === address),
        [systemState?.activeValidators],
    );

    return { isActiveValidator };
}
