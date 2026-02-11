// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export const shortenHash = (txHash: string) => {
    return `${txHash.slice(0, 6)}...${txHash.slice(-4)}`;
};
