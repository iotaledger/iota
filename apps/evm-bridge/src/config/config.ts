// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { z } from 'zod';
import { Config, envSchema } from './config.schema';

function loadConfig(): Config {
    const rawEvmBridgeConfig = import.meta.env.VITE_EVM_BRIDGE_CONFIG;

    let evmBridgeConfig: Record<string, unknown> = {};

    try {
        evmBridgeConfig = JSON.parse(rawEvmBridgeConfig);
    } catch (error) {
        throw new Error(
            `Failed to parse IOTA EVM Bridge config JSON env var! ${(error as Error)?.message}`,
        );
    }

    try {
        return envSchema.parse(evmBridgeConfig);
    } catch (error) {
        if (error instanceof z.ZodError) {
            const missingVars = error.issues
                .map((issue) => `${issue.path.join('.')}: ${issue.message}`)
                .join('\n');
            throw new Error(`Missing required configuration:\n${missingVars}`);
        }

        throw error;
    }
}

export const CONFIG = getNetwork(getDefaultNetwork());

function getNetwork(network: string) {
    const config = loadConfig();
    return config[network];
}

export function getDefaultNetwork(): string {
    return import.meta.env.VITE_EVM_BRIDGE_DEFAULT_NETWORK;
}
