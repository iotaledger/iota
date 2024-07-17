// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const developer = [
			'developer/developer',
			{
				type: 'category',
				label: 'Getting Started',
				collapsed: false,
				link: {
					type: 'doc',
					id: 'developer/getting-started',
				},
				items: [
					'developer/getting-started/iota-environment',
					'developer/getting-started/iota-install',
					'developer/getting-started/connect',
					'developer/getting-started/local-network',
					'developer/getting-started/get-address',
					'developer/getting-started/get-coins',
					'developer/getting-started/graphql-rpc',
					{
						type: 'category',
						label: 'Your First IOTA dApp',
						collapsed: false,
						link: {
							type: 'doc',
							id: 'developer/getting-started/first-app/first-app',
						},
						items: [
							'developer/getting-started/first-app/write-package',
							'developer/getting-started/first-app/build-test',
							'developer/getting-started/first-app/publish',
							'developer/getting-started/first-app/debug',
							'developer/getting-started/first-app/client-tssdk',
						],
					},
				],
			},
			{
				type: 'category',
				label: 'From Solidity/EVM to Move',
				collapsed: true,
				link: {
					type: 'doc',
					id: 'developer/evm-to-move',
				},
				items: [
					'developer/evm-to-move/why-move',
					'developer/evm-to-move/tooling-apis',
					'developer/evm-to-move/creating-token',
					'developer/evm-to-move/creating-nft',
				],
			},
			{
				type: 'category',
				label: 'IOTA 101',
				link: {
					type: 'doc',
					id: 'developer/iota-101',
				},
				items: [
					{
						type: 'category',
						label: 'Move Overview',link: {
							type: 'doc',
							id: 'developer/iota-101/iota-move-concepts/iota-move-concepts',
						},
						items: [
							'developer/iota-101/iota-move-concepts/strings',
							'developer/iota-101/iota-move-concepts/collections',
							'developer/iota-101/iota-move-concepts/init',
							'developer/iota-101/iota-move-concepts/entry-functions',
							'developer/iota-101/iota-move-concepts/one-time-witness',
							{
								type: 'category',
								label: 'Package Upgrades',
								link: {
									type: 'doc',
									id: 'developer/iota-101/iota-move-concepts/packages/packages',
								},
								items: [
									'developer/iota-101/iota-move-concepts/packages/upgrade',
									'developer/iota-101/iota-move-concepts/packages/custom-policies',
								],
							},
							{
								type: 'category',
								label: 'Patterns',
								link: {
									type: 'doc',
									id: 'developer/iota-101/iota-move-concepts/patterns',
								},
								items: [
									'developer/iota-101/iota-move-concepts/patterns/capabilities',
									'developer/iota-101/iota-move-concepts/patterns/witness',
									'developer/iota-101/iota-move-concepts/patterns/transferrable-witness',
									'developer/iota-101/iota-move-concepts/patterns/hot-potato',
									'developer/iota-101/iota-move-concepts/patterns/id-pointer',
								],
							},
							'developer/iota-101/iota-move-concepts/conventions',
						],
					},
					'developer/graphql-rpc',
					{
						type: 'category',
						label: 'Object Model',
						items:[
							'developer/iota-101/objects/object-model',
							'developer/iota-101/objects/shared-owned',
							{
								type: 'category',
								label: 'Object Ownership',
								link: {
									type: 'doc',
									id: 'developer/iota-101/objects/object-ownership/object-ownership',
								},
								items: [
									'developer/iota-101/objects/object-ownership/address-owned',
									'developer/iota-101/objects/object-ownership/immutable',
									'developer/iota-101/objects/object-ownership/shared',
									'developer/iota-101/objects/object-ownership/wrapped',
								],
							},
							{
								type: 'category',
								label: 'Dynamic Fields',
								link: {
									type: 'doc',
									id: 'developer/iota-101/objects/dynamic-fields/dynamic-fields',
								},
								items: ['developer/iota-101/objects/dynamic-fields/tables-bags'],
							},
							{
								type: 'category',
								label: 'Transfers',
								link: {
									type: 'doc',
									id: 'developer/iota-101/objects/transfers/transfers',
								},
								items: ['developer/iota-101/objects/transfers/custom-rules',
									'developer/iota-101/objects/transfers/transfer-to-object'],
							},
							'developer/iota-101/objects/events',
							'developer/iota-101/objects/versioning',
						]
					},
					{
						type: 'category',
						label: 'Transactions',
						link: {
							type: 'doc',
							id: 'developer/iota-101/transactions/transactions',
						},
						items:[
							'developer/iota-101/transactions/sign-and-send-txn',
							{
								type:'category',
								label: 'Sponsored Transactions',
								link: {
									type: 'doc',
									id: 'developer/iota-101/transactions/sponsor-txn',
								},
								items:['developer/iota-101/transactions/sponsored-transactions']
							},
							'developer/iota-101/transactions/gas-smashing',
							{
								type: 'category',
								label: 'Working with PTBs',
								items: [
									'developer/iota-101/transactions/ptb/prog-txn-blocks',
									'developer/iota-101/transactions/ptb/building-ptb',
									'developer/iota-101/transactions/ptb/coin-mgt',
									'developer/iota-101/transactions/ptb/simulating-refs',
								],
							},
						]
					},
					{
						type: 'category',
						label: 'Create Coins and Tokens',
						link: {
							type: 'doc',
							id: 'developer/iota-101/create-coin/create-coin',
						},
						items: [
							'developer/iota-101/create-coin/regulated',
							'developer/iota-101/create-coin/in-game-token',
							'developer/iota-101/create-coin/loyalty',
						],
					},
					'developer/iota-101/create-nft',
					'developer/iota-101/using-events',
					'developer/iota-101/access-time',
				],
			},
			{
				type: 'category',
				label: 'Cryptography',
				link: {
					type: 'doc',
					id:'developer/cryptography/explanations/cryptography',
				},
				items: [
					{
						type: 'category',
						label: 'Explanations',
						items: [
							'developer/cryptography/explanations/cryptography',
							{
								type: 'category',
								label: 'Transaction Authentication',
								link: {
									type: 'doc',
									id: 'developer/cryptography/explanations/transaction-auth',
								},
								items: [
									'developer/cryptography/explanations/transaction-auth/keys-addresses',
									'developer/cryptography/explanations/transaction-auth/signatures',
									'developer/cryptography/explanations/transaction-auth/multisig',
									'developer/cryptography/explanations/transaction-auth/offline-signing',
									'developer/cryptography/explanations/transaction-auth/intent-signing',
								],
							},
							'developer/cryptography/explanations/system/checkpoint-verification',
						],
					},
					{
						type: 'category',
						label: 'How To',
						items: [
							'developer/cryptography/how-to/cryptography',
							'developer/cryptography/how-to/signing',
							'developer/cryptography/how-to/groth16',
							'developer/cryptography/how-to/hashing',
							'developer/cryptography/how-to/ecvrf',]
					}
				],
			},
			{
				type: 'category',
				label: 'Advanced Topics',
				link: {
					type: 'doc',
					id: 'developer/advanced',
				},
				items: [
					/*{
						type: 'category',
						label: 'Efficient Smart Contracts',
						link: {
							type: 'doc',
							id: 'developer/advanced/efficient-smart-contracts',
						},
						items: ['developer/advanced/min-gas-fees'],
					},*/
					'developer/advanced/graphql-migration',
					'developer/advanced/move-2024-migration',
					'developer/advanced/asset-tokenization',
					'developer/advanced/custom-indexer',
					'developer/advanced/stardust-on-move',
				],
			},
			{
				type: 'category',
				label: 'App Examples',
				link: {
					type: 'doc',
					id: 'developer/app-examples',
				},
				items: [
					'developer/app-examples/blackjack',
					'developer/app-examples/coin-flip',
					'developer/app-examples/e2e-counter',
					'developer/app-examples/plinko',
					'developer/app-examples/recaptcha',
					'developer/app-examples/tic-tac-toe',
					{
						type: 'category',
						label: 'Trustless Token Swap',
						link: {
							type: 'doc',
							id: 'developer/app-examples/trustless-token-swap',
						},
						items: [
							'developer/app-examples/trustless-token-swap/backend',
							'developer/app-examples/trustless-token-swap/indexer-api',
							'developer/app-examples/trustless-token-swap/frontend',
						],
					},
				],
			},
			{
				type:'category',
				label: 'Standards',
				items: [
					'developer/standards/standards',
					'developer/standards/coin',
					'developer/standards/coin-manager',
					{
						type: 'category',
						label: 'Closed-Loop Token',
						link: {
							type: 'doc',
							id: 'developer/standards/closed-loop-token',
						},
						items: [
							'developer/standards/closed-loop-token/action-request',
							'developer/standards/closed-loop-token/token-policy',
							'developer/standards/closed-loop-token/spending',
							'developer/standards/closed-loop-token/rules',
							'developer/standards/closed-loop-token/coin-token-comparison',
						],
					},
					'developer/standards/kiosk',
					'developer/standards/kiosk-apps',
					'developer/standards/display',
					'developer/standards/wallet-standard',
				]
			},
			'developer/dev-cheat-sheet',
			{
				type:'category',
				label: 'Integrate Your Exchange',
				items:[
					'developer/exchange-integration/exchange-integration',

					{
						type: 'category',
						label: 'Migrating IOTA/Shimmer Stardust',
						link: {
							type: 'doc',
							id: 'developer/stardust/stardust-migration',
						},
						items: [
							'developer/stardust/exchanges',
							'developer/stardust/move-models',
							'developer/stardust/addresses',
							'developer/stardust/units',
							'developer/stardust/migration-process',
							'developer/stardust/claiming',
							'developer/stardust/vested',
							'developer/stardust/testing',
							'developer/stardust/if-tools',
							'developer/stardust/faq',
							'developer/stardust/advanced',
						],
					},
				]
			},
		]
;
module.exports = developer;
