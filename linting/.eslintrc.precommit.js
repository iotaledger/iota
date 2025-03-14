// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module.exports = {
    plugins: ['license-check'],
    rules: {
        'license-check/license-check': 'error',
    },
    overrides: [
        {
            files: [
                'sdk/create-dapp/templates/**/*',
            ],
            rules: {
                'license-check/license-check': 'off',
            },
        },
    ],
};
