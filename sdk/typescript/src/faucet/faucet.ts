// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { NetworkId } from '../client/index.js';
import { getNetwork } from '../client/index.js';

export class FaucetRateLimitError extends Error {}

type FaucetCoinInfo = {
    amount: number;
    id: string;
    transferTxDigest: string;
};

type FaucetResponse = {
    transferredGasObjects: FaucetCoinInfo[];
    error?: string | null;
};

type BatchFaucetResponse = {
    task?: string | null;
    error?: string | null;
};

type BatchSendStatusType = {
    status: 'INPROGRESS' | 'SUCCEEDED' | 'DISCARDED';
    transferred_gas_objects: { sent: FaucetCoinInfo[] };
};

type BatchStatusFaucetResponse = {
    status: BatchSendStatusType;
    error?: string | null;
};

type FaucetRequest = {
    host: string;
    path: string;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    body?: Record<string, any>;
    headers?: HeadersInit;
    method: 'GET' | 'POST';
};

async function faucetRequest({ host, path, body, headers, method }: FaucetRequest) {
    const endpoint = new URL(path, host).toString();
    const res = await fetch(endpoint, {
        method,
        body: body ? JSON.stringify(body) : undefined,
        headers: {
            'Content-Type': 'application/json',
            ...(headers || {}),
        },
    });

    if (res.status === 429) {
        throw new FaucetRateLimitError(
            `Too many requests from this client have been sent to the faucet. Please retry later`,
        );
    }

    try {
        const parsed = await res.json();
        if (parsed.error) {
            throw new Error(`Faucet returns error: ${parsed.error}`);
        }
        return parsed;
    } catch (e) {
        throw new Error(
            `Encountered error when parsing response from faucet, error: ${e}, status ${res.status}, response ${res}`,
        );
    }
}

export async function requestIotaFromFaucetV0(input: {
    host: string;
    recipient: string;
    headers?: HeadersInit;
}): Promise<FaucetResponse> {
    return faucetRequest({
        host: input.host,
        path: '/gas',
        body: {
            FixedAmountRequest: {
                recipient: input.recipient,
            },
        },
        headers: input.headers,
        method: 'POST',
    });
}

export async function requestIotaFromFaucetV1(input: {
    host: string;
    recipient: string;
    headers?: HeadersInit;
}): Promise<BatchFaucetResponse> {
    return faucetRequest({
        host: input.host,
        path: '/v1/gas',
        body: {
            FixedAmountRequest: {
                recipient: input.recipient,
            },
        },
        headers: input.headers,
        method: 'POST',
    });
}

export async function getFaucetRequestStatus(input: {
    host: string;
    taskId: string;
    headers?: HeadersInit;
}): Promise<BatchStatusFaucetResponse> {
    return faucetRequest({
        host: input.host,
        path: `/v1/status/${input.taskId}`,
        headers: input.headers,
        method: 'GET',
    });
}

const DEFAULT_MAX_FAUCET_ATTEMPTS = 20;
const DEFAULT_FAUCET_POLL_DELAY_MS = 1500;

/**
 * Submits a V1 faucet request for the given recipient and polls until the
 * request succeeds, is discarded, or the attempt limit is reached.
 *
 * @param input.host - Base URL of the faucet service.
 * @param input.recipient - Address to receive the tokens.
 * @param input.headers - Optional HTTP headers forwarded to every faucet call.
 * @param input.maxAttempts - Maximum number of status-poll attempts before
 *   giving up (default: 20).
 * @param input.delayMs - Milliseconds to wait between poll attempts
 *   (default: 1500).
 * @returns The total amount of gas transferred, or `undefined` if the faucet
 *   response contained no coin info.
 * @throws {Error} When the request is discarded, the attempt limit is exceeded,
 *   or the faucet returns an error at any stage.
 */
export async function requestIotaFromFaucet(input: {
    host: string;
    recipient: string;
    headers?: HeadersInit;
    maxAttempts?: number;
    delayMs?: number;
}): Promise<number | undefined> {
    const maxAttempts = input.maxAttempts ?? DEFAULT_MAX_FAUCET_ATTEMPTS;
    const delayMs = input.delayMs ?? DEFAULT_FAUCET_POLL_DELAY_MS;

    const { error, task: taskId } = await requestIotaFromFaucetV1({
        recipient: input.recipient,
        host: input.host,
        headers: input.headers,
    });

    if (error || !taskId) {
        throw new Error(error ?? 'Failed, task id not found.');
    }

    let currentStatus = 'INPROGRESS';
    let attempts = 0;
    while (currentStatus === 'INPROGRESS') {
        const {
            status: { status, transferred_gas_objects },
            error,
        } = await getFaucetRequestStatus({
            host: input.host,
            taskId,
            headers: input.headers,
        });

        currentStatus = status;

        if (currentStatus === 'DISCARDED' || error || attempts > maxAttempts) {
            throw new Error(error ?? status ?? 'Something went wrong');
        }

        if (currentStatus === 'SUCCEEDED') {
            return transferred_gas_objects?.sent.reduce((total, { amount }) => total + amount, 0);
        }

        attempts += 1;
        await new Promise((resolve) => setTimeout(resolve, delayMs));
    }

    throw new Error('Something went wrong');
}

export function getFaucetHost(network: NetworkId): string {
    const requestedNetwork = getNetwork(network);

    if (!requestedNetwork.faucet) {
        throw new Error(`Unknown network: ${network}`);
    }

    return requestedNetwork.faucet;
}
