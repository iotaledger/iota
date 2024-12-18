// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import { decodeIotaPrivateKey } from '@iota/iota-sdk/cryptography';
import type { Keypair } from '@iota/iota-sdk/cryptography';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import type { Transaction } from '@iota/iota-sdk/transactions';

import { DeepBookClient } from '../src/index.js'; // Adjust path according to new structure
import type { BalanceManager } from '../src/types/index.js';

export class DeepBookMarketMaker extends DeepBookClient {
	keypair: Keypair;
	iotaClient: IotaClient;

	constructor(
		keypair: string | Keypair,
		env: 'testnet' | 'mainnet',
		balanceManagers?: { [key: string]: BalanceManager },
		adminCap?: string,
	) {
		let resolvedKeypair: Keypair;

		if (typeof keypair === 'string') {
			resolvedKeypair = DeepBookMarketMaker.#getSignerFromPK(keypair);
		} else {
			resolvedKeypair = keypair;
		}

		const address = resolvedKeypair.toIotaAddress();

		super({
			address: address,
			env: env,
			client: new IotaClient({
				url: getFullnodeUrl(env),
			}),
			balanceManagers: balanceManagers,
			adminCap: adminCap,
		});

		this.keypair = resolvedKeypair;
		this.iotaClient = new IotaClient({
			url: getFullnodeUrl(env),
		});
	}

	static #getSignerFromPK = (privateKey: string) => {
		const { schema, secretKey } = decodeIotaPrivateKey(privateKey);
		if (schema === 'ED25519') return Ed25519Keypair.fromSecretKey(secretKey);

		throw new Error(`Unsupported schema: ${schema}`);
	};

	signAndExecute = async (tx: Transaction) => {
		return this.iotaClient.signAndExecuteTransaction({
			transaction: tx,
			signer: this.keypair,
			options: {
				showEffects: true,
				showObjectChanges: true,
			},
		});
	};

	getActiveAddress() {
		return this.keypair.getPublicKey().toIotaAddress();
	}

	// Example of a flash loan transaction
	// Borrow 1 DEEP from DEEP_IOTA pool
	// Swap 0.5 DBUSDC for IOTA in IOTA_DBUSDC pool, pay with deep borrowed
	// Swap IOTA back to DEEP
	// Return 1 DEEP to DEEP_IOTA pool
	flashLoanExample = async (tx: Transaction) => {
		const borrowAmount = 1;
		const [deepCoin, flashLoan] = tx.add(this.flashLoans.borrowBaseAsset('DEEP_IOTA', borrowAmount));

		// Execute trade using borrowed DEEP
		const [baseOut, quoteOut, deepOut] = tx.add(
			this.deepBook.swapExactQuoteForBase({
				poolKey: 'IOTA_DBUSDC',
				amount: 0.5,
				deepAmount: 1,
				minOut: 0,
				deepCoin: deepCoin,
			}),
		);

		tx.transferObjects([baseOut, quoteOut, deepOut], this.getActiveAddress());

		// Execute second trade to get back DEEP for repayment
		const [baseOut2, quoteOut2, deepOut2] = tx.add(
			this.deepBook.swapExactQuoteForBase({
				poolKey: 'DEEP_IOTA',
				amount: 10,
				deepAmount: 0,
				minOut: 0,
			}),
		);

		tx.transferObjects([quoteOut2, deepOut2], this.getActiveAddress());

		// Return borrowed DEEP
		const loanRemain = tx.add(
			this.flashLoans.returnBaseAsset('DEEP_IOTA', borrowAmount, baseOut2, flashLoan),
		);
		tx.transferObjects([loanRemain], this.getActiveAddress());
	};

	placeLimitOrderExample = (tx: Transaction) => {
		tx.add(
			this.deepBook.placeLimitOrder({
				poolKey: 'IOTA_DBUSDC',
				balanceManagerKey: 'MANAGER_1',
				clientOrderId: '123456789',
				price: 1,
				quantity: 10,
				isBid: true,
				// orderType default: no restriction
				// selfMatchingOption default: allow self matching
				// payWithDeep default: true
			}),
		);
	};
}
