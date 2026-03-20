// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface ControllerCap {
    objectId: string;
    weight: number;
}

export interface IdentityController extends ControllerCap {
    objectType?: string | null;
    owner?: string | null;
    ownerType?: string;
    error?: Error | unknown;
    isError: boolean;
}
