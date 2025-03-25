// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import {
    type IotaSystemStateSummary,
    type IotaSystemStateSummaryV1,
    type IotaValidatorSummary,
    type Network,
} from '@iota/iota-sdk/client';
import { Feature } from '../enums';
import { useFeatureEnabledByNetwork, useNetwork } from '../hooks';

interface UniversalIotaSystemStateFields {
    epoch: string;
    activeValidators: IotaValidatorSummary[];
    committeeMembers: IotaValidatorSummary[];
    atRiskValidators: [string, string][];
    pendingActiveValidatorsSize: string;
    isLoading: boolean;
    isError: boolean;
    isPending: boolean;
    isSuccess: boolean;
}

export function getUniversalIotaSystemStateFields(): UniversalIotaSystemStateFields {
    const [network] = useNetwork();
    const hasTopStakersCommitteeSelection = useFeatureEnabledByNetwork(
        Feature.TopStakersCommitteeSelection,
        network as Network,
    );

    const {
        data: system,
        isLoading,
        isError,
        isPending,
        isSuccess,
    } = hasTopStakersCommitteeSelection
        ? useIotaClientQuery('getLatestIotaSystemStateV2')
        : useIotaClientQuery('getLatestIotaSystemState');
    let activeValidators = [] as IotaValidatorSummary[];
    let committeeMembers = [] as IotaValidatorSummary[];
    let epoch: string = '';
    let atRiskValidators: [string, string][] = [];
    let pendingActiveValidatorsSize: string = '';
    if (system) {
        if (hasTopStakersCommitteeSelection) {
            const iotaSystemState = system as IotaSystemStateSummary;
            if ('V2' in iotaSystemState) {
                activeValidators = iotaSystemState.V2.activeValidators;
                committeeMembers = iotaSystemState.V2.committeeMembers.map(
                    (committeeMemberIndex) => {
                        return activeValidators[Number(committeeMemberIndex)];
                    },
                );
                epoch = iotaSystemState.V2.epoch;
                atRiskValidators = iotaSystemState.V2.atRiskValidators;
                pendingActiveValidatorsSize = iotaSystemState.V2.pendingActiveValidatorsSize;
            } else {
                activeValidators = iotaSystemState.V1.activeValidators;
                committeeMembers = iotaSystemState.V1.activeValidators;
                epoch = iotaSystemState.V1.epoch;
                atRiskValidators = iotaSystemState.V1.atRiskValidators;
                pendingActiveValidatorsSize = iotaSystemState.V1.pendingActiveValidatorsSize;
            }
        } else {
            const iotaSystemState = system as IotaSystemStateSummaryV1;
            activeValidators = iotaSystemState?.activeValidators;
            committeeMembers = iotaSystemState?.activeValidators;
            epoch = iotaSystemState?.epoch;
            atRiskValidators = iotaSystemState?.atRiskValidators;
            pendingActiveValidatorsSize = iotaSystemState?.pendingActiveValidatorsSize;
        }
    }
    return {
        epoch,
        activeValidators,
        committeeMembers,
        atRiskValidators,
        pendingActiveValidatorsSize,
        isLoading,
        isError,
        isPending,
        isSuccess,
    };
}
