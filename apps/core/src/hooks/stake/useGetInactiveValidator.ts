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
        queryKey: [inactivePoolsId, validatorAddress],
        async queryFn() {
            if (!inactivePoolsId || !validatorAddress) {
                throw Error('Missing params');
            }
            const inactiveValidators = await iotaClient.getDynamicFields({
                parentId: normalizeIotaAddress(inactivePoolsId),
            });
            const pendingInactiveValidatorsData = await Promise.all(
                inactiveValidators.data.map(
                    async (validator) =>
                        await getInactiveValidatorsMetadata(iotaClient, validator.objectId),
                ),
            );
            const validator = pendingInactiveValidatorsData.find(
                (validator) => validator?.validatorAddress === validatorAddress,
            );
            return validator ?? null;
        },
        enabled: !!inactivePoolsId && !!validatorAddress,
    });
}
