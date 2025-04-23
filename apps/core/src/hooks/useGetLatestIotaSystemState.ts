// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import type { IotaSystemStateSummary, IotaValidatorSummary } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';

export type IotaSystemStateSummaryCompat = {
    activeValidators: IotaValidatorSummary[];
    committeeMembers: IotaValidatorSummary[];
    atRiskValidators: [string, string][];
    epoch: string;
    epochDurationMs: string;
    epochStartTimestampMs: string;
    pendingActiveValidatorsId: string;
    pendingActiveValidatorsSize: string;
    protocolVersion: string;
    inactivePoolsId: string;
};

export function useGetLatestIotaSystemState() {
    const iotaClient = useIotaClient();
    return useQuery<IotaSystemStateSummaryCompat>({
        queryKey: ['system', 'state'],
        async queryFn() {
            const protocolConfig = await iotaClient.getProtocolConfig();
            const isV2Supported = Number(protocolConfig.maxSupportedProtocolVersion) >= 5;

            const iotaSystemStateSummary: IotaSystemStateSummary = isV2Supported
                ? await iotaClient.getLatestIotaSystemStateV2()
                : {
                      V1: await iotaClient.getLatestIotaSystemState(),
                  };

            return 'V2' in iotaSystemStateSummary
                ? {
                      activeValidators: iotaSystemStateSummary.V2.activeValidators,
                      committeeMembers: iotaSystemStateSummary.V2.committeeMembers.map(
                          (committeeMemberIndex) =>
                              iotaSystemStateSummary.V2.activeValidators[
                                  Number(committeeMemberIndex)
                              ],
                      ),
                      atRiskValidators: iotaSystemStateSummary.V2.atRiskValidators,
                      epoch: iotaSystemStateSummary.V2.epoch,
                      epochDurationMs: iotaSystemStateSummary.V2.epochDurationMs,
                      epochStartTimestampMs: iotaSystemStateSummary.V2.epochStartTimestampMs,
                      pendingActiveValidatorsId:
                          iotaSystemStateSummary.V2.pendingActiveValidatorsId,
                      pendingActiveValidatorsSize:
                          iotaSystemStateSummary.V2.pendingActiveValidatorsSize,
                      protocolVersion: iotaSystemStateSummary.V2.protocolVersion,
                      inactivePoolsId: iotaSystemStateSummary.V2.inactivePoolsId,
                  }
                : {
                      activeValidators: iotaSystemStateSummary.V1.activeValidators,
                      committeeMembers: iotaSystemStateSummary.V1.activeValidators,
                      atRiskValidators: iotaSystemStateSummary.V1.atRiskValidators,
                      epoch: iotaSystemStateSummary.V1.epoch,
                      epochDurationMs: iotaSystemStateSummary.V1.epochDurationMs,
                      epochStartTimestampMs: iotaSystemStateSummary.V1.epochStartTimestampMs,
                      pendingActiveValidatorsId:
                          iotaSystemStateSummary.V1.pendingActiveValidatorsId,
                      pendingActiveValidatorsSize:
                          iotaSystemStateSummary.V1.pendingActiveValidatorsSize,
                      protocolVersion: iotaSystemStateSummary.V1.protocolVersion,
                      inactivePoolsId: iotaSystemStateSummary.V1.inactivePoolsId,
                  };
        },
    });
}
