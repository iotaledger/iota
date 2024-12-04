// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export type BalanceChange = {
    coinType: string;
    amount: string;
    unRecognizedToken: boolean;
    recipient?: string;
    owner?: string;
};
