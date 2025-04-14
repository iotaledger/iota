// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import { useCallback } from 'react';

export function useIsValidatorCommitteeMember() {
    const { data: systemState } = useIotaClientQuery('getLatestIotaSystemState');

    const isCommitteeMember = useCallback(
        (address: string) =>
            systemState?.committeeMembers.some((member) => member.iotaAddress === address),
        [systemState?.committeeMembers],
    );

    return { isCommitteeMember };
}
