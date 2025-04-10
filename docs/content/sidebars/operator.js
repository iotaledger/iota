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
            'operator/full-node/source',
            {
                type: 'category',
                label: 'Full Node Configuration',
                link: {
                    type: 'doc',
                    id: 'operator/full-node/configuration',
                },
                items: [
                    'operator/full-node/configs/genesis',
                    'operator/full-node/configs/network',
                    'operator/full-node/configs/pruning',
                    'operator/full-node/configs/snapshots',
                    'operator/full-node/configs/archives',
                ],
            },
            'operator/full-node/monitoring',
            'operator/full-node/troubleshooting',
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
                label: 'Validator Node Configuration',
                link: {
                    type: 'doc',
                    id: 'operator/validator-node/configuration',
                },
                items: [
                    'operator/validator-node/configs/genesis',
                    'operator/validator-node/configs/network',
                    'operator/validator-node/configs/pruning',
                    'operator/validator-node/configs/snapshots',
                    'operator/validator-node/configs/archives',
                ],
            },
            'operator/validator-node/validator-tokenomics',
            'operator/validator-node/cli-validator-command',
            'operator/validator-node/monitoring',
            'operator/ssfn_guide',
            'operator/activate_DoS_protection_for_validators',
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
    // 'operator/security-releases',
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
