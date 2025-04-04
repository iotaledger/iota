// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback } from 'react';
import { useGetLatestIotaSystemState } from '@iota/core';

export function useIsValidatorCommitteeMember() {
    const { data: systemState } = useGetLatestIotaSystemState();

    const isCommitteeMember = useCallback(
        (address: string) =>
            systemState?.committeeMembers.some((member) => member.iotaAddress === address),
        [systemState?.committeeMembers],
    );

    return { isCommitteeMember };
}
