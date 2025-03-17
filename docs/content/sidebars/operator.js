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
    'operator/validator-tool',
    'operator/iota-tool',
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
        label: 'Gas Station',
        link: {
            type: 'doc',
            id: 'operator/gas-station/gas-station',
        },
        items: [
            {
                type: 'category',
                label: 'Architecture',
                link: {
                    type: 'doc',
                    id: 'operator/gas-station/architecture/architecture',
                },
                items: [
                    {
                        type: 'doc',
                        label: 'Components',
                        id: 'operator/gas-station/architecture/components',
                    },
                    {
                        type: 'doc',
                        label: 'Features',
                        id: 'operator/gas-station/architecture/features',
                    },
                ],
            },
            'operator/gas-station/deployment/deployment',
            'operator/gas-station/api-reference/api-reference',
        ],
    },
];
module.exports = operator;
