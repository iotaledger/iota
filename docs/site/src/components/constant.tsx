export const Networks = {
  iota: {
    baseToken: 'IOTA Token',
    protocol: 'Stardust',
    httpRestApi: 'https://api.stardust-mainnet.iotaledger.net',
    eventApi: 'wss://api.stardust-mainnet.iotaledger.net:443 (MQTT 3.1, /mqtt)',
    permaNodeApi: 'https://chronicle.stardust-mainnet.iotaledger.net',
    explorer: 'https://explorer.iota.org/mainnet',
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
      core: {
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
    baseToken: 'Testnet Token (no value)',
    protocol: 'Stardust',
    httpRestApi: 'https://api.testnet.iotaledger.net',
    eventApi: 'wss://api.testnet.iotaledger.net:443 (MQTT 3.1, /mqtt)',
    permaNodeApi: 'https://chronicle.testnet.iotaledger.net',
    faucet: 'https://faucet.testnet.iotaledger.net',
    explorer: 'https://explorer.iota.org/iota-testnet',
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
  iota_2_testnet: {
    baseToken: 'Testnet Token (no value)',
    protocol: 'IOTA 2.0',
    httpRestApi: 'https://api.nova-testnet.iotaledger.net/',
    eventApi: 'wss://api.nova-testnet.iotaledger.net:443 (MQTT 3.1, /mqtt)',
    permaNodeApi: 'https://chronicle.nova-testnet.iotaledger.net',
    explorer: 'https://explorer.iota.org/iota2-testnet',
    faucet: 'https://faucet.nova-testnet.iotaledger.net',
  },
  iota_move: {
    baseToken: 'IOTA Token',
    jsonRpcUrl: 'jsonRpcUrl placeholder',
    jsonRpcWebsocketUrl: 'jsonRpcWebsocketUrl placeholder',
    indexerRpc: 'indexerRpc placeholder',
    graphqlRpc: 'graphqlRpc placeholder',
    faucetUrl: 'faucetUrl placeholder',
    explorerUrl: 'https://explorer.rebased.iota.org'
  },
  iota_move_testnet: {
    baseToken: 'IOTA Token (no value)',
    jsonRpcUrl: 'https://api.testnet.iota.cafe',
    jsonRpcWebsocketUrl: 'wss://api.testnet.iota.cafe',
    indexerRpc: 'https://indexer.testnet.iota.cafe',
    graphqlRpc: 'https://graphql.testnet.iota.cafe',
    faucetUrl: 'https://faucet.testnet.iota.cafe',
    explorerUrl: 'https://explorer.rebased.iota.org/?network=testnet'
  },
  iota_move_devnet: {
    baseToken: 'IOTA Token (no value)',
    jsonRpcUrl: 'https://api.devnet.iota.cafe',
    jsonRpcWebsocketUrl: 'wss://api.devnet.iota.cafe',
    indexerRpc: 'https://indexer.devnet.iota.cafe',
    graphqlRpc: 'https://graphql.devnet.iota.cafe',
    faucetUrl: 'https://faucet.devnet.iota.cafe',
    explorerUrl: 'https://explorer.rebased.iota.org/?network=devnet'
  },
  iota_localnet: {
    baseToken: "IOTA Token",
    jsonRpcUrl: 'http://127.0.0.1:9000',
    jsonRpcWebsocketUrl: 'ws://127.0.0.1:9000',
    indexerRpc: 'http://127.0.0.1:9124',
    graphqlRpc: 'http://127.0.0.1:8000',
    faucetUrl: 'http://127.0.0.1:9123/gas'
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
  httpRestApi: string;
  eventApi: string;
  permaNodeApi: string;
  faucetUrl?: string;
  explorer: string;
  evm: AddEthereumChainParameter;
  evmCustom: {
    chainAddress: string;
    aliasId: string;
    blastApiUrls?: Array<string | object>;
    ankrApiUrls?: Array<string | object>;
    toolkit?: Toolkit;
    api?: string;
  };
}

export interface MoveProps {
  jsonRpcUrl: string;
  jsonRpcWebsocketUrl: string;
  indexerRpc: string;
  graphqlRpc: string;
  faucetUrl: string;
  explorerUrl?: string;
};