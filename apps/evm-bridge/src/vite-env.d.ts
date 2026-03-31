// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// <reference types="vite/client" />

declare const COMMIT_REV: string;

interface ImportMetaEnv {
    readonly VITE_EVM_BRIDGE_DEFAULT_NETWORK: string;
    readonly VITE_EVM_BRIDGE_CONFIG: string;
}

interface ImportMeta {
    readonly env: ImportMetaEnv;
}
