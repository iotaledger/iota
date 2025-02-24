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
];
module.exports = operator;
