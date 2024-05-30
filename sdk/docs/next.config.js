// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const withNextra = require('nextra')({
	theme: 'nextra-theme-docs',
	themeConfig: './theme.config.jsx',
});

module.exports = withNextra({
	redirects: () => {
		return [
			{
				source: '/',
				destination: '/typescript',
				statusCode: 302,
			},
		];
	},
});
