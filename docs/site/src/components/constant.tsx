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
    explorer: 'https://explorer.iota.org',
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
        'wss://ws.json-rpc.evm.iotaledger.net',
      ],
      blockExplorerUrls: ['https://explorer.evm.iota.org'],
    },
    evmCustom: {
      chainAddress:
        'iota1pzt3mstq6khgc3tl0mwuzk3eqddkryqnpdxmk4nr25re2466uxwm28qqxu5',
      aliasId:
        '0x971dc160d5ae8c457f7eddc15a39035b6190130b4dbb5663550795575ae19db5',
      ankrApiUrls: ['https://rpc.ankr.com/iota_evm'],
      blastApiUrls: [
        'https://iota-mainnet-evm.public.blastapi.io',
        'wss://iota-mainnet-evm.public.blastapi.io',
        {
          'Archive RPC':
            'https://iota-mainnet-evm.blastapi.io/e7596858-fc63-4a54-8727-b885a2af4ec8',
        },
      ],
      toolkit: {
        url: 'https://evm-toolkit.evm.iotaledger.net',
        hasFaucet: false,
      },
      api: 'https://api.evm.iotaledger.net',
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
    explorer: 'https://explorer.rebased.iota.org/?network=testnet',
    evm: {
      chainId: '0x433',
      chainName: 'IOTA EVM Testnet',
      nativeCurrency: {
        name: 'IOTA',
        symbol: 'IOTA',
        decimals: 18,
      },
      rpcUrls: [
        'https://json-rpc.evm.testnet.iota.cafe',
        'wss://ws.json-rpc.evm.testnet.iotaledger.net',
      ],
      blockExplorerUrls: ['https://explorer.evm.testnet.iota.cafe'],
    },
    evmCustom: {
      chainAddress:
        'tst1pzxsrr7apqkdzz633dyntmvxwtyvk029p39te5j0m33q6946h7akzv663zu',
      aliasId:
        '0x8d018fdd082cd10b518b4935ed8672c8cb3d450c4abcd24fdc620d16babfbb61',
      ankrApiUrls: ['https://rpc.ankr.com/iota_evm_testnet'],
      blastApiUrls: [
        'https://iota-testnet-evm.public.blastapi.io',
        'wss://iota-testnet-evm.public.blastapi.io',
        {
          'Archive RPC':
            'https://iota-testnet-evm.blastapi.io/e7596858-fc63-4a54-8727-b885a2af4ec8',
        },
      ],
      toolkit: {
        url: 'https://evm-toolkit.evm.testnet.iotaledger.net',
        hasFaucet: false,
      },
      api: 'https://api.evm.testnet.iotaledger.net',
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
    explorer: 'https://explorer.rebased.iota.org/?network=devnet',
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
    explorer: 'https://explorer.rebased.iota.org/?network=http://127.0.0.1:9000',
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
  explorer: string;
  evm?: AddEthereumChainParameter;
  evmCustom?: {
    chainAddress: string;
    aliasId: string;
    blastApiUrls?: Array<string | object>;
    ankrApiUrls?: Array<string | object>;
    toolkit?: Toolkit;
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