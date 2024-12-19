// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface UID {
    id: string;
}

export interface Balance {
    value: bigint;
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
