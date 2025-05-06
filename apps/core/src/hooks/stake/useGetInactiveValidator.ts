// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { useIotaClient, useIotaClientQuery } from '@iota/dapp-kit';

import { InactiveValidatorData } from '../../types';
import { getInactiveValidatorsMetadata } from '../../utils';

export function useGetInactiveValidator(validatorAddress: string): InactiveValidatorData | null {
    const iotaClient = useIotaClient();
    const { data } = useIotaClientQuery('getLatestIotaSystemState');
    const inactivePoolsId = data?.inactivePoolsId;
    const queryResult = useQuery({
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
            return pendingInactiveValidatorsData;
        },
        enabled: !!inactivePoolsId && !!validatorAddress,
    });

    if (queryResult.isLoading || queryResult.isError || !queryResult.data) {
        return null;
    }

    const validatorData = queryResult.data.find(
        (validator) => validator?.validatorAddress === validatorAddress,
    );

    return validatorData || null;
}
