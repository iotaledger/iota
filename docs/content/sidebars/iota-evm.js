const apiDocs = require("../iota-evm/references/openapi/sidebar");

const iotaEvm = [
    'iota-evm/iota-evm',
    {
        type: 'category',
        label: 'Getting Started',
        items: [
            {
                type: 'doc',
                label: 'Languages & VMs',
                id: 'iota-evm/getting-started/languages-and-vms',
            },
            'iota-evm/getting-started/quick-start',
            'iota-evm/getting-started/compatibility',
            {
                type: 'doc',
                label: 'Networks & Chains',
                id: 'iota-evm/getting-started/networks-and-chains',
            },
            {
                type: 'doc',
                label: 'Contracts',
                id: 'iota-evm/getting-started/contracts',
            },
            {
                type: 'doc',
                label: 'Tools',
                id: 'iota-evm/getting-started/tools',
            },
        ],
    },
    {
        type: 'category',
        label: 'Tools',
        items: [
            {
                type: 'category',
                label: 'IOTA EVM',
                collapsed: false,
                link: {
                    type: 'generated-index',
                    title: 'IOTA EVM Tools',
                    slug: '/iota-evm/tools/iota',
                },
                items: [
                    {
                        label: 'Explorer',
                        type: 'link',
                        href: 'https://explorer.evm.iota.org',
                    },
                    {
                        label: 'IOTA EVM Bridge',
                        type: 'link',
                        href: 'https://evm-bridge.iota.org',
                    },
                ],
            },
            {
                type: 'category',
                label: 'IOTA EVM Testnet',
                collapsed: false,
                link: {
                    type: 'generated-index',
                    title: 'IOTA Testnet EVM Tools',
                    slug: '/iota-evm/tools/iota-testnet',
                },
                items: [
                    {
                        label: 'Explorer',
                        type: 'link',
                        href: 'https://explorer.evm.testnet.iota.org',
                    },
                    {
                        label: 'IOTA EVM Bridge & Faucet',
                        type: 'link',
                        href: 'https://testnet.evm-bridge.iota.org',
                    },
                ],
            },
            {
                label: 'RPC Providers',
                type: 'doc',
                id: 'iota-evm/tools/rpcProviders',
            },
            {
                label: 'Oracles',
                type: 'doc',
                id: 'iota-evm/tools/oracles',
            },
            {
                label: 'Subgraphs',
                type: 'doc',
                id: 'iota-evm/tools/subgraphs',
            },
            {
                label: 'IOTA Safe Wallet',
                type: 'doc',
                id: 'iota-evm/tools/safe',
            },
            {
                label: 'Multicall3',
                type: 'doc',
                id: 'iota-evm/tools/multicall',
            }
        ],
    },
    {
        type: 'category',
        label: 'How To',
        items: [
            'iota-evm/how-tos/introduction',
            {
                type: 'doc',
                label: 'Send Funds from L1 to L2',
                id: 'iota-evm/how-tos/send-funds-from-L1-to-L2',
            },
            {
                type: 'doc',
                label: 'Create a Basic Contract',
                id: 'iota-evm/how-tos/create-a-basic-contract',
            },
            {
                type: 'doc',
                label: 'Deploy a Smart Contract',
                id: 'iota-evm/how-tos/deploy-a-smart-contract',
            },
            {
                type: 'doc',
                label: 'Create Custom Tokens - ERC20',
                id: 'iota-evm/how-tos/ERC20',
            },
            {
                type: 'doc',
                label: 'Create NFTs - ERC721',
                id: 'iota-evm/how-tos/ERC721',
            },
            {
                type: 'doc',
                label: 'Test Smart Contracts',
                id: 'iota-evm/how-tos/test-smart-contracts',
            },
            {
                type: 'category',
                label: 'Interact with the Core Contracts',
                items: [
                    {
                        type: 'doc',
                        label: 'Introduction',
                        id: 'iota-evm/how-tos/core-contracts/introduction',
                    },
                    {
                        type: 'category',
                        label: 'Basics',
                        items: [
                            {
                                type: 'doc',
                                label: 'Get Native Assets Balance',
                                id: 'iota-evm/how-tos/core-contracts/basics/get-balance',
                            },
                            {
                                type: 'category',
                                label: 'Allowance',
                                items: [
                                    {
                                        type: 'doc',
                                        label: 'Allow',
                                        id: 'iota-evm/how-tos/core-contracts/basics/allowance/allow',
                                    },
                                    {
                                        type: 'doc',
                                        label: 'Get Allowance',
                                        id: 'iota-evm/how-tos/core-contracts/basics/allowance/get-allowance',
                                    },
                                    {
                                        type: 'doc',
                                        label: 'Take Allowance',
                                        id: 'iota-evm/how-tos/core-contracts/basics/allowance/take-allowance',
                                    },
                                ],
                            },
                            {
                                type: 'doc',
                                label: 'Send Assets to L1',
                                id: 'iota-evm/how-tos/core-contracts/basics/send-assets-to-l1',
                            },
                        ],
                    },
                    {
                        type: 'category',
                        label: 'Objects',
                        items: [
                            {
                                label: 'Introduction',
                                type: 'doc',
                                id: 'iota-evm/how-tos/core-contracts/objects/introduction',
                            },
                            {
                                type: 'doc',
                                label: 'Get Objects Owned by an Account',
                                id: 'iota-evm/how-tos/core-contracts/objects/get-L2-objects',
                            },
                        ],
                    },
                    {
                        type: 'doc',
                        label: 'Get Randomness on L2',
                        id: 'iota-evm/how-tos/core-contracts/get-randomness-on-l2',
                    },
                    {
                        type: 'doc',
                        label: 'Call and Call View',
                        id: 'iota-evm/how-tos/core-contracts/call-view',
                    },
                ],
            },
        ],
    },
    {
        type: 'category',
        label: 'Tutorials',
        items: [
            {
                type: 'category',
                label: 'Cross-chain NFT Marketplace',
                items: [
                    {
                        type: 'doc',
                        label: 'Part I',
                        id: 'iota-evm/tutorials/cross-chain-nft-marketplace-part-1',
                    },
                    {
                        type: 'doc',
                        label: 'Part II',
                        id: 'iota-evm/tutorials/cross-chain-nft-marketplace-part-2',
                    },
                ],
            },
            {
                type: 'category',
                label: 'Defi Lend Borrow',
                items: [
                    {
                        type: 'doc',
                        label: 'Part I',
                        id: 'iota-evm/tutorials/defi-lend-borrow-tutorial-part-1',
                    },
                    {
                        type: 'doc',
                        label: 'Part II',
                        id: 'iota-evm/tutorials/defi-lend-borrow-tutorial-part-2',
                    },
                ],
            },
            {
                type: 'doc',
                label: 'Yield Farming',
                id: 'iota-evm/tutorials/defi-yield-farming',
            },
        ],
    },
    {
        type: 'category',
        label: 'Explanations',
        items: [
            {
                type: 'doc',
                label: 'Anatomy of a Smart Contract',
                id: 'iota-evm/explanations/smart-contract-anatomy',
            },
            {
                type: 'doc',
                label: 'Sandbox Interface',
                id: 'iota-evm/explanations/sandbox',
            },
            {
                type: 'doc',
                label: 'Calling a Smart Contract',
                id: 'iota-evm/explanations/invocation',
            },
            {
                type: 'doc',
                label: 'State, Transitions and State Anchoring',
                id: 'iota-evm/explanations/states',
            },
            {
                type: 'doc',
                label: 'State manager',
                id: 'iota-evm/explanations/state_manager',
            },
            {
                type: 'doc',
                label: 'Validators and Access Nodes',
                id: 'iota-evm/explanations/validators',
            },
            {
                type: 'doc',
                label: 'Consensus',
                id: 'iota-evm/explanations/consensus',
            },
            {
                type: 'doc',
                label: 'How Accounts Work',
                id: 'iota-evm/explanations/how-accounts-work',
            },
            {
                type: 'doc',
                label: 'Move Packages',
                id: 'iota-evm/explanations/move-anchor',
            },
            {
                type: 'link',
                label: 'Core Contracts',
                href: '/iota-evm/references/core-contracts/overview',
            },
        ],
    },
    {
        type: 'category',
        label: 'Test with iota-evm/solo',
        items: [
            {
                label: 'Getting Started',
                id: 'iota-evm/solo/getting-started',
                type: 'doc',
            },
            {
                type: 'category',
                label: 'How To',
                items: [
                    {
                        type: 'doc',
                        label: 'First Example',
                        id: 'iota-evm/solo/how-tos/first-example',
                    },
                    {
                        type: 'doc',
                        label: 'The L1 Ledger',
                        id: 'iota-evm/solo/how-tos/the-l1-ledger',
                    },
                    {
                        type: 'doc',
                        label: 'Call a View',
                        id: 'iota-evm/solo/how-tos/view-sc',
                    },
                    {
                        type: 'doc',
                        label: 'Accounts',
                        id: 'iota-evm/solo/how-tos/the-l2-ledger',
                    },
                ],
            },
        ],
    },
    {
        type: 'category',
        label: 'Operator Guides',
        link: {
            type: 'doc',
            id: 'iota-evm/operator/how-tos/running-a-node',
        },
        items: [
            {
                type: 'category',
                label: 'How To',
                collapsed: false,
                items: [
                    {
                        type: 'doc',
                        id: 'iota-evm/operator/how-tos/running-a-node',
                        label: 'Run a Node',
                    },
                    {
                        type: 'doc',
                        id: 'iota-evm/operator/how-tos/running-an-access-node',
                        label: 'Run an Access Node',
                    },
                    {
                        id: 'iota-evm/operator/how-tos/wasp-cli',
                        label: 'Configure wasp-cli',
                        type: 'doc',
                    },
                    {
                        id: 'iota-evm/operator/how-tos/setting-up-a-chain',
                        label: 'Set Up a Chain',
                        type: 'doc',
                    },
                    {
                        id: 'iota-evm/operator/how-tos/chain-management',
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
                        id: 'iota-evm/operator/reference/configuration',
                    },
                    {
                        type: 'doc',
                        id: 'iota-evm/operator/reference/metrics',
                    },
                ],
            },
        ],
    },
    {
        type: 'category',
        label: 'References',
        items: [
            'iota-evm/references/json-rpc-spec',
            {
                type: 'category',
                label: 'Magic Contract',
                items: [
                    {
                        type: 'autogenerated',
                        dirName: 'iota-evm/references/magic-contract',
                    },
                ],
            },
            {
                type: 'category',
                label: 'Core Contracts',
                items: [
                    {
                        type: 'doc',
                        label: 'Overview',
                        id: 'iota-evm/references/core-contracts/overview',
                    },
                    {
                        type: 'doc',
                        label: 'root',
                        id: 'iota-evm/references/core-contracts/root',
                    },
                    {
                        type: 'doc',
                        label: 'accounts',
                        id: 'iota-evm/references/core-contracts/accounts',
                    },
                    {
                        type: 'doc',
                        label: 'blocklog',
                        id: 'iota-evm/references/core-contracts/blocklog',
                    },
                    {
                        type: 'doc',
                        label: 'governance',
                        id: 'iota-evm/references/core-contracts/governance',
                    },
                    {
                        type: 'doc',
                        label: 'errors',
                        id: 'iota-evm/references/core-contracts/errors',
                    },
                    {
                        type: 'doc',
                        label: 'EVM',
                        id: 'iota-evm/references/core-contracts/evm',
                    },
                    {
                        type: 'doc',
                        label: 'Types',
                        id: 'iota-evm/references/core-contracts/types',
                    },
                ],
            },
            {
                type: 'category',
                label: 'ISC Utilities',
                items: [
                    {
                        type: 'autogenerated',
                        dirName: 'iota-evm/references/iscutils',
                    },
                ],
            },
            {
                type: 'category',
                label: 'WASP API',
                items: apiDocs,
            },
        ],
    },
];

module.exports = iotaEvm;
