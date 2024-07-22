// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const aboutIota = [
	'about-iota/about-iota',
	'about-iota/components',
	{
		type: 'category',
		label: 'IOTA Architecture',
		collapsed: false,
		link: {
			type: 'doc',
			id: 'about-iota/iota-architecture',
		},
		items: [
			'about-iota/iota-architecture/iota-security',
			'about-iota/iota-architecture/transaction-lifecycle',
			'about-iota/iota-architecture/consensus',
			'about-iota/iota-architecture/indexer-functions',
			'about-iota/iota-architecture/epochs',
			'about-iota/iota-architecture/protocol-upgrades',
			'about-iota/iota-architecture/staking-rewards',
		],
	},
	{
		type: 'category',
		label: 'Execution Architecture',
		items: [
			'about-iota/execution-architecture/iota-execution',
			'about-iota/execution-architecture/adapter',
			'about-iota/execution-architecture/natives',
		],
	},
	{
		type: 'category',
		label: 'Tokenomics',
		collapsed: false,
		link: {
			type: 'doc',
			id: 'about-iota/tokenomics',
		},
		items: [
			'about-iota/tokenomics/proof-of-stake',
			'about-iota/tokenomics/validators-staking',
			'about-iota/tokenomics/staking-unstaking',
			'about-iota/tokenomics/iota-coin',
			'about-iota/tokenomics/gas-pricing',
			'about-iota/tokenomics/gas-in-iota',
		],
	},
	'about-iota/research-papers',
];
module.exports = aboutIota;
