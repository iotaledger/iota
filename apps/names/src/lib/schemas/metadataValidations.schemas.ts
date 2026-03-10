// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { z } from 'zod';

export const METADATA_KEYS = [
    { key: 'twitter/x', label: 'Twitter/X', allowedKey: 'twitterX' },
    { key: 'discord', label: 'Discord', allowedKey: 'discord' },
    { key: 'github', label: 'Github', allowedKey: 'github' },
    { key: 'email', label: 'Email', allowedKey: 'email' },
    { key: 'btc', label: 'BTC', allowedKey: 'btc' },
    { key: 'eth', label: 'ETH', allowedKey: 'eth' },
    { key: 'ltc', label: 'LTC', allowedKey: 'ltc' },
    { key: 'doge', label: 'DOGE', allowedKey: 'doge' },
    { key: 'sol', label: 'SOL', allowedKey: 'sol' },
    { key: 'sui', label: 'SUI', allowedKey: 'sui' },
    { key: 'website', label: 'Website', allowedKey: 'website' },
    { key: 'ipfs', label: 'IPFS', allowedKey: 'ipfs' },
    { key: 'arweave', label: 'Arweave', allowedKey: 'arweave' },
] as const;

// Zod schemas for validation mapped by allowedKey
export type AllowedKey = (typeof METADATA_KEYS)[number]['allowedKey'];
export const SCHEMAS: Record<AllowedKey, z.ZodString> = {
    twitterX: z
        .string()
        .min(2, 'Twitter handle too short')
        .max(16, 'Twitter handle too long')
        .regex(/^@/, 'Twitter handle must start with @')
        .regex(/^@[a-zA-Z0-9_]+$/, 'Invalid Twitter handle format'),
    discord: z
        .string()
        .regex(/^[a-z0-9._]{2,32}(#[0-9]{4})?$/i, 'Discord format: username or username#1234'),
    github: z
        .string()
        // allow optional leading @, then GitHub username rules
        .regex(
            /^@?[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,37}[a-zA-Z0-9])?$/,
            'Invalid GitHub username format',
        ),
    email: z.string().email('Invalid email format').max(254, 'Email too long'),
    btc: z.string().refine(
        (v) => {
            const legacy = /^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$/;
            const bech32 = /^bc1[a-z0-9]{39,59}$/;
            const taproot = /^bc1p[a-z0-9]{58}$/;
            return legacy.test(v) || bech32.test(v) || taproot.test(v);
        },
        { message: 'Invalid Bitcoin address format' },
    ),
    eth: z
        .string()
        .regex(/^0x/, 'ETH address must start with 0x')
        .regex(/^0x[0-9a-fA-F]{40}$/i, 'Invalid Ethereum address format'),
    ltc: z.string().refine(
        (v) => {
            const legacy = /^[LM3][a-km-zA-HJ-NP-Z1-9]{25,33}$/;
            const bech32 = /^ltc1[a-z0-9]{39,59}$/;
            return legacy.test(v) || bech32.test(v);
        },
        { message: 'Invalid Litecoin address format' },
    ),
    doge: z
        .string()
        .regex(/^[DA][5-9A-HJ-NP-U][1-9A-HJ-NP-Za-km-z]{32}$/, 'Invalid Dogecoin address format'),
    sol: z
        .string()
        .regex(
            /^[1-9A-HJ-NP-Za-km-z]{32,44}$/,
            'Invalid Solana address format (32-44 base58 characters)',
        ),
    sui: z
        .string()
        .regex(/^0x/, 'SUI address must start with 0x')
        .regex(/^0x[0-9a-fA-F]{64}$/, 'Invalid SUI address format (66 characters total)'),
    website: z.string().superRefine((v, ctx) => {
        let urlToValidate = v;
        if (!v.startsWith('http://') && !v.startsWith('https://')) {
            urlToValidate = `https://${v}`;
        }
        try {
            const url = new URL(urlToValidate);
            if (!url.hostname || url.hostname.length === 0) {
                ctx.addIssue({ code: z.ZodIssueCode.custom, message: 'Invalid website hostname' });
                return;
            }
            const hostnameRegex =
                /^[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$/;
            if (!hostnameRegex.test(url.hostname)) {
                ctx.addIssue({ code: z.ZodIssueCode.custom, message: 'Invalid website format' });
                return;
            }
            const parts = url.hostname.split('.');
            if (parts.length < 2) {
                ctx.addIssue({
                    code: z.ZodIssueCode.custom,
                    message: 'Website must include a valid domain (e.g., example.com)',
                });
                return;
            }
            const tld = parts[parts.length - 1];
            if (tld.length < 2 || !/^[a-zA-Z]+$/.test(tld)) {
                ctx.addIssue({
                    code: z.ZodIssueCode.custom,
                    message: 'Invalid top-level domain (TLD)',
                });
            }
        } catch (error) {
            ctx.addIssue({
                code: z.ZodIssueCode.custom,
                message: `Invalid URL format: ${error instanceof Error ? error.message : 'Invalid URL format'}`,
            });
        }
    }),
    ipfs: z.string().superRefine((v, ctx) => {
        const cidv0Regex = /^Qm[1-9A-HJ-NP-Za-km-z]{44}$/;
        const cidv1Regex = /^b[a-z2-7]{58}$/;
        const ipfsUrlRegex =
            /^(ipfs:\/\/|https?:\/\/.+\/ipfs\/)(Qm[1-9A-HJ-NP-Za-km-z]{44}|b[a-z2-7]{58})$/;
        if (v.includes('ipfs://') || v.includes('/ipfs/')) {
            if (!ipfsUrlRegex.test(v)) {
                ctx.addIssue({ code: z.ZodIssueCode.custom, message: 'Invalid IPFS URL format' });
            }
            return;
        }
        if (!cidv0Regex.test(v) && !cidv1Regex.test(v)) {
            ctx.addIssue({
                code: z.ZodIssueCode.custom,
                message: 'Invalid IPFS hash. Use CIDv0 (Qm...) or CIDv1 (b...) format',
            });
        }
    }),
    arweave: z.string().superRefine((v, ctx) => {
        const arweaveIdRegex = /^[A-Za-z0-9_-]{43}$/;
        const arweaveUrlRegex =
            /^https?:\/\/(arweave\.net|ar\.io|arweave\.dev)\/([A-Za-z0-9_-]{43})$/;
        if (v.includes('arweave.net/') || v.includes('ar.io/') || v.includes('arweave.dev/')) {
            if (!arweaveUrlRegex.test(v)) {
                ctx.addIssue({
                    code: z.ZodIssueCode.custom,
                    message: 'Invalid Arweave URL format',
                });
            }
            return;
        }
        if (!arweaveIdRegex.test(v)) {
            ctx.addIssue({
                code: z.ZodIssueCode.custom,
                message: 'Invalid Arweave transaction ID (43 characters, base64url)',
            });
        }
    }),
};
