// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export type ReceiverInputFormValues = {
    to: string;
    resolvedAddress: string | null;
};

export type SendNftFormValues = ReceiverInputFormValues;

export type SendTokenFormValues = ReceiverInputFormValues & {
    amount: string;
};
