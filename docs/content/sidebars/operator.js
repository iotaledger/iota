// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const operator = [
    'operator/index',
    {
        type: 'category',
        label: 'Full Node',
        items: [
            'operator/full-node/overview',            
            'operator/full-node/docker',
            'operator/full-node/systemd',
            {
                type: 'category',
                label: 'Full Node Configuration',
                link: {
                    type: 'doc',
                    id: 'operator/full-node/configuration',
                },
                items: [
                    'operator/full-node/configs/network',
                    'operator/full-node/configs/genesis',
                    'operator/full-node/configs/pruning',
                    'operator/full-node/configs/snapshots',
                    'operator/full-node/configs/archives',
                ],
            },
            'operator/full-node/monitoring',
        ],
    },
    {
        type: 'category',
        label: 'Validator Node',
        items: [
            'operator/validator-node/overview',            
            'operator/validator-node/docker',
            'operator/validator-node/systemd',
            {
                type: 'category',
                label: 'Full Node Configuration',
                items: [
                    'operator/validator-node/configs/network',
                    'operator/validator-node/configs/genesis',
                    'operator/validator-node/configs/pruning',
                    'operator/validator-node/configs/snapshots',
                    'operator/validator-node/configs/archives',
                ],
            },
            'operator/validator-node/validator-tasks',
            'operator/validator-node/validator-commands',
            'operator/validator-node/monitoring',
        ],
    },
    {
        type: 'category',
        label: 'Extensions',
        items: [
            'operator/extensions/indexer-functions',
        ],
    },
    'operator/data-management',
    'operator/security-releases',
    'operator/ssfn_guide',
];

module.exports = operator;
