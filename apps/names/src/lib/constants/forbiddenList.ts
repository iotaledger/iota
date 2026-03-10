// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export const FORBIDDEN_LIST: string[] =
    JSON.parse(process.env.NEXT_PUBLIC_NAMES_DAPP_FORBIDDEN_LIST || '[]') || [];
