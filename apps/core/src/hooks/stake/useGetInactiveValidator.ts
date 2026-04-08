// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { useIotaClient, useIotaClientQuery } from '@iota/dapp-kit';
import { getInactiveValidatorsMetadata } from '../../utils';

export function useGetInactiveValidator(validatorAddress: string) {
    const iotaClient = useIotaClient();
    const { data } = useIotaClientQuery('getLatestIotaSystemState');
    const inactivePoolsId = data?.inactivePoolsId;
    return useQuery({
        queryKey: ['inactive-validators', inactivePoolsId],
        async queryFn() {
            if (!inactivePoolsId) {
                throw Error('Missing params');
            }
            const inactiveValidators = await iotaClient.getDynamicFields({
                parentId: normalizeIotaAddress(inactivePoolsId),
            });
            return Promise.all(
                inactiveValidators.data.map((validator) =>
                    getInactiveValidatorsMetadata(iotaClient, validator.objectId),
                ),
            );
        },
        select(data) {
            return data.find((v) => v?.validatorAddress === validatorAddress) ?? null;
        },
        enabled: !!inactivePoolsId,
    });
}
