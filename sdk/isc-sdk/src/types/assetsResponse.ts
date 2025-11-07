import { z } from 'zod';

export const CoinJSONSchema = z.object({
    balance: z.string(),
    coinType: z.string(),
});

export const AssetsResponseSchema = z.object({
    baseTokens: z.string(),
    nativeTokens: z.array(CoinJSONSchema),
});

export type AssetsResponse = z.infer<typeof AssetsResponseSchema>;
