export const Networks: Record<string, NetworkProps> = {
  iota: {
    baseToken: 'IOTA Token',
    protocol: 'Rebased',
    rpc: {
      json: {
        core: 'https://api.mainnet.iota.cafe',
        websocket: 'wss://api.mainnet.iota.cafe',
        indexer: 'https://indexer.mainnet.iota.cafe',
      },
      graphql: 'https://graphql.mainnet.iota.cafe',
    },
    explorer: 'https://explorer.iota.org/',
    evm: {
      chainId: '0x2276',
      chainName: 'IOTA EVM',
      nativeCurrency: {
        name: 'IOTA',
        symbol: 'IOTA',
        decimals: 18,
      },
      rpcUrls: [
        'https://json-rpc.evm.iotaledger.net',
      ],
      blockExplorerUrls: ['https://explorer.evm.iota.org'],
    },
    evmCustom: {
      chainId:
        '0x0dc448563a2c54778215b3d655b0d9f8f69f06cf80a4fc9eada72e96a49e409d',
      packageId:
        '0x1b33a3cf7eb5dde04ed7ae571db1763006811ff6b7bb35b3d1c780de153af9dd',
      ankrApiUrls: ['https://rpc.ankr.com/iota_evm'],
      blastApiUrls: [
        'https://iota-mainnet-evm.public.blastapi.io',
        'wss://iota-mainnet-evm.public.blastapi.io',
        {
          'Archive RPC':
            'https://iota-mainnet-evm.blastapi.io/e7596858-fc63-4a54-8727-b885a2af4ec8',
        },
      ],
      bridge: {
        url: 'https://evm-bridge.iota.org',
        hasFaucet: false,
      },
      api: '',
    },
  },
  iota_testnet: {
    baseToken: 'IOTA Token (no value)',
    protocol: 'Rebased',
    rpc: {
      json: {
        core: 'https://api.testnet.iota.cafe',
        websocket: 'wss://api.testnet.iota.cafe',
        indexer: 'https://indexer.testnet.iota.cafe',
      },
      graphql: 'https://graphql.testnet.iota.cafe',
    },
    faucet: 'https://faucet.testnet.iota.cafe',
    explorer: {
      url: 'https://explorer.rebased.iota.org/',
      query: '?network=testnet',
    },
    evm: {
      chainId: '0x434',
      chainName: 'IOTA EVM Testnet',
      nativeCurrency: {
        name: 'IOTA',
        symbol: 'IOTA',
        decimals: 18,
      },
      rpcUrls: [
        'https://json-rpc.evm.testnet.iota.cafe',
      ],
      blockExplorerUrls: ['https://explorer.evm.testnet.iota.cafe/'],
    },
    evmCustom: {
      chainId:
        '0x2f11f5ea9d3c093c9cc2e329cf92e05aa00ac052ada96c4c14a2f6869a7cbcaf',
      packageId:
        '0x1e6e060b87f55acc0a7632acab9cf5712ff01643f8577c9a6f99ebd1010e3f4c',
      ankrApiUrls: ['https://rpc.ankr.com/iota_evm_testnet'],
      blastApiUrls: [
        'https://iota-testnet-evm.public.blastapi.io',
        'wss://iota-testnet-evm.public.blastapi.io',
        {
          'Archive RPC':
            'https://iota-testnet-evm.blastapi.io/e7596858-fc63-4a54-8727-b885a2af4ec8',
        },
      ],
      bridge: {
        url: 'https://testnet.evm-bridge.iota.org',
        hasFaucet: true,
      },
      api: '',
    },
  },
  iota_devnet: {
    baseToken: 'IOTA Token (no value)',
    protocol: 'Rebased',
    rpc: {
      json: {
        core: 'https://api.devnet.iota.cafe',
        websocket: 'wss://api.devnet.iota.cafe',
        indexer: 'https://indexer.devnet.iota.cafe',
      },
      graphql: 'https://graphql.devnet.iota.cafe',
    },
    faucet: 'https://faucet.devnet.iota.cafe',
    explorer: {
      url: 'https://explorer.rebased.iota.org/',
      query: '?network=devnet',
    },
  },
  iota_localnet: {
    baseToken: "IOTA Token (no value)",
    protocol: 'Custom',
    rpc: {
      json: {
        core: 'http://127.0.0.1:9000',
        websocket: 'ws://127.0.0.1:9000',
        indexer: 'http://127.0.0.1:9124',
      },
      graphql: 'http://127.0.0.1:8000',
    },
    faucet: 'http://127.0.0.1:9123/gas',
    explorer: {
      url: 'https://explorer.rebased.iota.org/',
      query: '?network=http://127.0.0.1:9000',
    }
  },
};

export interface Toolkit {
  url: string;
  hasFaucet: boolean;
}

export interface AddEthereumChainParameter {
  chainId: string; // A 0x-prefixed hexadecimal string
  chainName: string;
  nativeCurrency?: {
    name: string;
    symbol: string; // 2-6 characters long
    decimals: number;
  };
  rpcUrls?: string[];
  blockExplorerUrls?: string[];
  iconUrls?: string[]; // Currently ignored.
}

export interface NetworkProps {
  baseToken: string;
  protocol: string;
  rpc: Rpc;
  faucet?: string;
  explorer: {
    url: string;
    query: string;
  } | string;
  evm?: AddEthereumChainParameter;
  evmCustom?: {
    chainId: string;
    packageId: string;
    blastApiUrls?: Array<string | object>;
    ankrApiUrls?: Array<string | object>;
    bridge?: Toolkit;
    api?: string;
  };
}

export interface Rpc {
  json: {
    core: string;
    indexer: string;
    websocket: string;
  };
  graphql: string;
}