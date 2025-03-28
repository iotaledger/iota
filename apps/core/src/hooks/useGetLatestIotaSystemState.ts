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
};

export function useGetLatestIotaSystemState() {
    const iotaClient = useIotaClient();
    return useQuery<IotaSystemStateSummaryCompat>({
        queryKey: ['system', 'state'],
        async queryFn() {
            const protocolConfig = await iotaClient.getProtocolConfig();
            const isV2Supported = Number(protocolConfig.maxSupportedProtocolVersion) >= 5;

            const iotaSystemStateSummay: IotaSystemStateSummary = isV2Supported
                ? await iotaClient.getLatestIotaSystemStateV2()
                : {
                      V1: await iotaClient.getLatestIotaSystemState(),
                  };

            return 'V2' in iotaSystemStateSummay
                ? {
                      activeValidators: iotaSystemStateSummay.V2.activeValidators,
                      committeeMembers: iotaSystemStateSummay.V2.committeeMembers.map(
                          (committeeMemberIndex) =>
                              iotaSystemStateSummay.V2.activeValidators[
                                  Number(committeeMemberIndex)
                              ],
                      ),
                      atRiskValidators: iotaSystemStateSummay.V2.atRiskValidators,
                      epoch: iotaSystemStateSummay.V2.epoch,
                      epochDurationMs: iotaSystemStateSummay.V2.epochDurationMs,
                      epochStartTimestampMs: iotaSystemStateSummay.V2.epochStartTimestampMs,
                      pendingActiveValidatorsId: iotaSystemStateSummay.V2.pendingActiveValidatorsId,
                      pendingActiveValidatorsSize:
                          iotaSystemStateSummay.V2.pendingActiveValidatorsSize,
                      protocolVersion: iotaSystemStateSummay.V2.protocolVersion,
                  }
                : {
                      activeValidators: iotaSystemStateSummay.V1.activeValidators,
                      committeeMembers: iotaSystemStateSummay.V1.activeValidators,
                      atRiskValidators: iotaSystemStateSummay.V1.atRiskValidators,
                      epoch: iotaSystemStateSummay.V1.epoch,
                      epochDurationMs: iotaSystemStateSummay.V1.epochDurationMs,
                      epochStartTimestampMs: iotaSystemStateSummay.V1.epochStartTimestampMs,
                      pendingActiveValidatorsId: iotaSystemStateSummay.V1.pendingActiveValidatorsId,
                      pendingActiveValidatorsSize:
                          iotaSystemStateSummay.V1.pendingActiveValidatorsSize,
                      protocolVersion: iotaSystemStateSummay.V1.protocolVersion,
                  };
        },
    });
}
