// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type {
    AddressMetrics,
    Checkpoint,
    DynamicFieldInfo,
    EpochInfo,
    EpochMetrics,
    IotaCallArg,
    IotaMoveNormalizedModule,
    IotaParsedData,
    IotaSystemStateSummaryV2,
    IotaTransaction,
    IotaValidatorSummary,
} from './generated.js';

export type EpochPage = {
    data: EpochInfo[];
    nextCursor: string | null;
    hasNextPage: boolean;
};

export type EpochMetricsPage = {
    data: EpochMetrics[];
    nextCursor: string | null;
    hasNextPage: boolean;
};

export type CheckpointPage = {
    data: Checkpoint[];
    nextCursor: string | null;
    hasNextPage: boolean;
};

export type AllEpochsAddressMetrics = AddressMetrics[];

export type MoveCallMetric = [
    {
        module: string;
        package: string;
        function: string;
    },
    string,
];

export type DynamicFieldPage = {
    data: DynamicFieldInfo[];
    nextCursor: string | null;
    hasNextPage: boolean;
};

export type IotaMoveNormalizedModules = Record<string, IotaMoveNormalizedModule>;

export type IotaMoveObject = Extract<IotaParsedData, { dataType: 'moveObject' }>;
export type IotaMovePackage = Extract<IotaParsedData, { dataType: 'package' }>;

export type ProgrammableTransaction = {
    transactions: IotaTransaction[];
    inputs: IotaCallArg[];
};

export type LatestIotaSystemStateSummary = {
    committeeMembers: IotaValidatorSummary[];
} & Omit<IotaSystemStateSummaryV2, 'committeeMembers'>;
