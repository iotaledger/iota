// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { z } from 'zod';

import { Config, envSchema } from './config.schema';

function loadConfig(): Config {
    const rawNamesDappConfig = process.env.NEXT_PUBLIC_NAMES_DAPP_CONFIG || '';

    let namesDappConfig: Record<string, unknown> = {};

    try {
        namesDappConfig = JSON.parse(rawNamesDappConfig);
    } catch (error) {
        throw new Error(
            `Failed to parse IOTA Names Dapp config JSON env var! ${(error as Error)?.message}, config: ${rawNamesDappConfig}`,
        );
    }

    try {
        return envSchema.parse(namesDappConfig);
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
    return process.env.NEXT_PUBLIC_NAMES_DAPP_DEFAULT_NETWORK || '';
}
