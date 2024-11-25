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
    'operator/indexer-functions',
    'operator/validator-committee',
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
            'operator/telemetry/iota-telemetry',
        ],
    },
];
module.exports = operator;
