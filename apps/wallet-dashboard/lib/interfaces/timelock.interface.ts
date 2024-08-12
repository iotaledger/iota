// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface UID {
    id: string;
}

export interface Balance {
    value: number;
}

export interface TimelockedObject {
    id: UID;
    locked: Balance; // TODO: extend to support other types of locked assets
    expirationTimestampMs: number;
    label?: string;
}

export interface TimelockedIotaResponse {
    id: UID;
    locked: string;
    expiration_timestamp_ms: string;
    label?: string;
}

export interface StakedIota {
    id: UID;
    poolId: string;
    stakeActivationEpoch: number;
    principal: Balance;
}

export interface TimelockedStakedIota {
    id: UID;
    stakedIota: StakedIota;
    expirationTimestampMs: number;
    label?: string | null | undefined;
}
