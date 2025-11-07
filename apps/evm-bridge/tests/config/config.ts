import { z } from 'zod';
import { envSchema, type Config } from '../../src/config/config.schema';
import * as dotenv from 'dotenv';
import path from 'path';

dotenv.config({
    path: [path.join(__dirname, '..', '..', '.env')],
});

export function loadConfig(): Config {
    const rawEvmBridgeConfig = process.env.VITE_EVM_BRIDGE_CONFIG;

    if (rawEvmBridgeConfig === undefined) {
        throw new Error('Missing IOTA EVM Bridge config JSON env var!');
    }

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
    return process.env.VITE_EVM_BRIDGE_DEFAULT_NETWORK as string;
}
