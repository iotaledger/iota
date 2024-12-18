// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useSignAndExecuteTransaction, useIotaClient } from '@iota/dapp-kit';
import { IotaClient, IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';

type Options = Omit<Parameters<IotaClient['getTransactionBlock']>[0], 'digest'> & {
	tx: Transaction;
};

type ExecuteResponse = { digest: string; rawEffects?: number[] };

type ExecuteCallback = ({
	bytes,
	signature,
}: {
	bytes: string;
	signature: string;
}) => Promise<ExecuteResponse>;

type ResponseCallback = (tx: IotaTransactionBlockResponse) => void | Promise<void>;
type Executor = (options: Options, then: ResponseCallback) => void;

type ExecutorResult = {
	mutate: Executor;
	status: string;
	isIdle: boolean;
	isPending: boolean;
	isSuccess: boolean;
	isError: boolean;
	isPaused: boolean;
};

/**
 * Hook encapsulating running a transaction, waiting for its effects
 * and then doing something with them.
 */
export function useExecutor({ execute }: { execute?: ExecuteCallback } = {}): ExecutorResult {
	const client = useIotaClient();
	const {
		mutate: signAndExecute,
		status,
		isIdle,
		isPending,
		isSuccess,
		isError,
		isPaused,
	} = useSignAndExecuteTransaction({ execute });

	const mutate: Executor = ({ tx, ...options }, then) => {
		signAndExecute(
			{
				transaction: tx,
			},
			{
				onSuccess: ({ digest }) => {
					client.waitForTransaction({ digest, ...options }).then(then);
				},

				onError: (error) => {
					console.error('Failed to execute transaction', tx, error);
				},
			},
		);
	};

	return {
		mutate,
		status,
		isIdle,
		isPending,
		isSuccess,
		isError,
		isPaused,
	};
}
