// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import {
    IotaSystemStateSummary,
    IotaSystemStateSummaryV1,
    IotaValidatorSummary,
    Network,
} from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';
import { useFeatureEnabledByNetwork, useNetwork } from '.';
import { Feature } from '../enums';

export function useGetCommitteeMembersInfo() {
    const [network] = useNetwork();

    const hasTopStakersCommitteeSelection = useFeatureEnabledByNetwork(
        Feature.TopStakersCommitteeSelection,
        network as Network,
    );

    const client = useIotaClient();

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['get-active-validators-info'],
        queryFn: async () => {
            let committeeMembers = [] as IotaValidatorSummary[];

            const system = hasTopStakersCommitteeSelection
                ? await client.getLatestIotaSystemState()
                : await client.getLatestIotaSystemStateV2();

            if (hasTopStakersCommitteeSelection) {
                const iotaSystemState = system as IotaSystemStateSummary;
                if ('V2' in iotaSystemState) {
                    const activeValidators = iotaSystemState.V2.activeValidators ?? [];
                    committeeMembers = iotaSystemState.V2.committeeMembers.map(
                        (committeeMemberIndex) => {
                            return activeValidators[Number(committeeMemberIndex)];
                        },
                    );
                } else {
                    committeeMembers = iotaSystemState.V1.activeValidators;
                }
            } else {
                const iotaSystemState = system as IotaSystemStateSummaryV1;
                committeeMembers = iotaSystemState?.activeValidators;
            }
            return committeeMembers;
        },
        staleTime: 10 * 60 * 1000, // 10 minutes
    });
}
