// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const concepts = [
	'concepts',
	'concepts/components',
	{
		type: 'category',
		label: 'App Developers',
		link: {
			type: 'doc',
			id: 'concepts/app-devs',
		},
		items: [
			{
				type: 'category',
				label: 'Object Model',
				link: {
					type: 'doc',
					id: 'concepts/object-model',
				},
				items: [
					{
						type: 'category',
						label: 'Object Ownership',
						link: {
							type: 'doc',
							id: 'concepts/object-ownership',
						},
						items: [
							'concepts/object-ownership/address-owned',
							'concepts/object-ownership/immutable',
							'concepts/object-ownership/shared',
							'concepts/object-ownership/wrapped',
						],
					},
					{
						type: 'category',
						label: 'Dynamic Fields',
						link: {
							type: 'doc',
							id: 'concepts/dynamic-fields',
						},
						items: ['concepts/dynamic-fields/tables-bags'],
					},
					{
						type: 'category',
						label: 'Transfers',
						link: {
							type: 'doc',
							id: 'concepts/transfers',
						},
						items: ['concepts/transfers/custom-rules', 'concepts/transfers/transfer-to-object'],
					},
					'concepts/versioning',
				],
			},
			{
				type: 'category',
				label: 'Move Overview',
				link: {
					type: 'doc',
					id: 'concepts/iota-move-concepts',
				},
				items: [
					'concepts/iota-move-concepts/strings',
					'concepts/iota-move-concepts/collections',
					'concepts/iota-move-concepts/init',
					'concepts/iota-move-concepts/entry-functions',
					'concepts/iota-move-concepts/one-time-witness',
					{
						type: 'category',
						label: 'Packages',
						link: {
							type: 'doc',
							id: 'concepts/iota-move-concepts/packages',
						},
						items: [
							'concepts/iota-move-concepts/packages/upgrade',
							'concepts/iota-move-concepts/packages/custom-policies',
							'concepts/iota-move-concepts/packages/automated-address-management',
						],
					},
					'concepts/iota-move-concepts/conventions',
				],
			},
			{
				type: 'category',
				label: 'Transactions',
				link: {
					type: 'doc',
					id: 'concepts/transactions',
				},
				items: [
					'concepts/transactions/prog-txn-blocks',
					'concepts/transactions/sponsored-transactions',
					'concepts/transactions/gas-smashing',
				],
			},
			'concepts/graphql-rpc',
		],
	},
	{
		type: 'category',
		label: 'Cryptography',
		link: {
			type: 'doc',
			id: 'concepts/cryptography',
		},
		items: [
			{
				type: 'category',
				label: 'Transaction Authentication',
				link: {
					type: 'doc',
					id: 'concepts/cryptography/transaction-auth',
				},
				items: [
					'concepts/cryptography/transaction-auth/keys-addresses',
					'concepts/cryptography/transaction-auth/signatures',
					'concepts/cryptography/transaction-auth/multisig',
					'concepts/cryptography/transaction-auth/offline-signing',
					'concepts/cryptography/transaction-auth/intent-signing',
				],
			},
			'concepts/cryptography/zklogin',
			'concepts/cryptography/system/checkpoint-verification',
			/*{
				type: 'category',
				label: 'System',
				link: {
					type: 'doc',
					id: 'concepts/cryptography/system',
				},
				items: [
					'concepts/cryptography/system/validator-signatures',
					'concepts/cryptography/system/intents-for-validation',
					'concepts/cryptography/system/checkpoint-verification',
				],
			},*/
		],
	},
	{
		type: 'category',
		label: 'Iota Architecture',
		link: {
			type: 'doc',
			id: 'concepts/iota-architecture',
		},
		items: [
			'concepts/iota-architecture/high-level',
			'concepts/iota-architecture/iota-storage',
			'concepts/iota-architecture/iota-security',
			'concepts/iota-architecture/transaction-lifecycle',
			'concepts/iota-architecture/consensus',
			'concepts/iota-architecture/indexer-functions',
			'concepts/iota-architecture/epochs',
			'concepts/iota-architecture/protocol-upgrades',
			'concepts/iota-architecture/data-management-things',
			'concepts/iota-architecture/staking-rewards',
		],
	},
	{
		type: 'category',
		label: 'Tokenomics',
		link: {
			type: 'doc',
			id: 'concepts/tokenomics',
		},
		items: [
			'concepts/tokenomics/proof-of-stake',
			'concepts/tokenomics/validators-staking',
			'concepts/tokenomics/staking-unstaking',
			'concepts/tokenomics/iota-coin',
			'concepts/tokenomics/iota-bridging',
			'concepts/tokenomics/storage-fund',
			'concepts/tokenomics/gas-pricing',
			'concepts/tokenomics/gas-in-iota',
		],
	},
	'concepts/iota-bridge',
	'concepts/research-papers',
];
module.exports = concepts;
