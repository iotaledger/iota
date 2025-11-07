import { z } from 'zod';

const HEX_REGEX = /^0x[0-9a-fA-F]+$/;

export const envSchema = z.record(
    z.object({
        L1: z.object({
            networkName: z.string(),
            rpcUrl: z.string().url(),
            faucetUrl: z.string().url().optional(),
            chainId: z.string(),
            packageId: z.string(),
        }),
        L2: z.object({
            chainName: z.string(),
            chainCurrency: z.string(),
            rpcUrl: z.string().url(),
            chainId: z.preprocess(
                (val) => (val !== undefined ? Number(val) : undefined),
                z.number().int().positive(),
            ),
            chainDecimals: z.preprocess(
                (val) => (val !== undefined ? Number(val) : undefined),
                z.number().int().positive(),
            ),
            chainExplorerName: z.string(),
            chainExplorerUrl: z.string(),
            wagmiAppName: z.string(),
            walletConnectProjectId: z.string(),
            iscContractAddress: z
                .string()
                .regex(
                    HEX_REGEX,
                    'Must be a valid hex string starting with 0x',
                ) as z.ZodType<`0x${string}`>,
            evmRpcUrl: z.string().url(),
        }),
    }),
);

export type Config = z.infer<typeof envSchema>;
