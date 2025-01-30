// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const operator = [
    'operator/operator',
    {
        type: 'category',
        label: 'IOTA Full Node Configuration',
        items: [
            'operator/iota-full-node/overview',
            'operator/iota-full-node/docker',
            'operator/iota-full-node/source',
            'operator/iota-full-node/monitoring-and-pruning',
        ],
    },
    'operator/validator-config',
    'operator/data-management',
    'operator/snapshots',
    'operator/archives',
    'operator/genesis',
    'operator/indexer-functions',
    {
        type: 'category',
        label: 'Validator Operation',
        items: [
            'operator/validator-operation/validator-tasks',
            'operator/validator-operation/ansible/README',
            'operator/validator-operation/docker/README',
            'operator/validator-operation/systemd/README',
        ]
    },
    'operator/validator-tools',
    {
        type: 'category',
        label: 'Node Monitoring and Metrics',
        items: [
            'operator/telemetry/telemetry-subscribers',
            'operator/telemetry/iota-metrics',
        ],
    },
    {
        type: 'category',
        label: 'IOTA EVM Network',
        link: {
            type: 'doc',
            id: 'operator/iota-evm/how-tos/running-a-node',
        },
        items: [
            {
                type: 'category',
                label: 'How To',
                collapsed: false,
                items: [
                    {
                        type: 'doc',
                        id: 'operator/iota-evm/how-tos/running-a-node',
                        label: 'Run a Node',
                    },
                    {
                        type: 'doc',
                        id: 'operator/iota-evm/how-tos/running-an-access-node',
                        label: 'Run an Access Node',
                    },
                    {
                        id: 'operator/iota-evm/how-tos/wasp-cli',
                        label: 'Configure wasp-cli',
                        type: 'doc',
                    },
                    {
                        id: 'operator/iota-evm/how-tos/setting-up-a-chain',
                        label: 'Set Up a Chain',
                        type: 'doc',
                    },
                    {
                        id: 'operator/iota-evm/how-tos/chain-management',
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
                        id: 'operator/iota-evm/reference/configuration',
                    },
                    {
                        type: 'doc',
                        id: 'operator/iota-evm/reference/metrics',
                    },
                ],
            },
        ],
    },
];
module.exports = operator;
