// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const operator = [
    'operator/operator',
    'operator/iota-full-node',
    'operator/validator-config',
    'operator/data-management',
    'operator/snapshots',
    'operator/archives',
    'operator/genesis',
    'operator/validator-committee',
    'operator/validator-tasks',
    'operator/node-tools',
    {
        type: 'category',
        label: 'IOTA Chains Node',
        link: {
            type: 'doc',
            id: 'guides/operator/iota-chains-node/how-tos/running-a-node',
        },
        items: [
            {
                type: 'category',
                label: 'How To',
                collapsed: false,
                items: [
                    {
                        type: 'doc',
                        id: 'guides/operator/iota-chains-node/how-tos/running-a-node',
                        label: 'Run a Node',
                    },
                    {
                        type: 'doc',
                        id: 'guides/operator/iota-chains-node/how-tos/running-an-access-node',
                        label: 'Run an Access Node',
                    },
                    {
                        id: 'guides/operator/iota-chains-node/how-tos/wasp-cli',
                        label: 'Configure wasp-cli',
                        type: 'doc',
                    },
                    {
                        id: 'guides/operator/iota-chains-node/how-tos/setting-up-a-chain',
                        label: 'Set Up a Chain',
                        type: 'doc',
                    },
                    {
                        id: 'guides/operator/iota-chains-node/how-tos/chain-management',
                        label: 'Manage a Chain',
                        type: 'doc',
                    },
                ],
            },
            {
                type: 'category',
                label: 'Reference',
                items: [
                    {
                        type: 'doc',
                        id: 'guides/operator/iota-chains-node/reference/configuration',
                    },
                    {
                        type: 'doc',
                        id: 'guides/operator/iota-chains-node/reference/metrics',
                    },
                ],
            },
        ],
    },
];
module.exports = operator;
