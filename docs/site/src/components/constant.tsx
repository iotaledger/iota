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
    },
    evmCustom: {
      chainAddress:
        'iota1pzt3mstq6khgc3tl0mwuzk3eqddkryqnpdxmk4nr25re2466uxwm28qqxu5',
      aliasId:
        '0x971dc160d5ae8c457f7eddc15a39035b6190130b4dbb5663550795575ae19db5',
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
        'https://json-rpc.evm.testnet.iotaledger.net',
        'wss://ws.json-rpc.evm.testnet.iotaledger.net',
      ],
      blockExplorerUrls: ['https://explorer.evm.testnet.iotaledger.net'],
    },
    evmCustom: {
      chainAddress:
        'tst1pzxsrr7apqkdzz633dyntmvxwtyvk029p39te5j0m33q6946h7akzv663zu',
      aliasId:
        '0x8d018fdd082cd10b518b4935ed8672c8cb3d450c4abcd24fdc620d16babfbb61',
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
  shimmer: {
    baseToken: 'Shimmer Token',
    protocol: 'Stardust',
    httpRestApi: 'https://api.shimmer.network',
    eventApi: 'wss://api.shimmer.network:443/api/mqtt/v1 (MQTT 3.1)',
    permaNodeApi: 'https://chronicle.shimmer.network',
    explorer: 'https://explorer.shimmer.network/shimmer',
    evm: {
      chainId: '0x94',
      chainName: 'ShimmerEVM',
      nativeCurrency: {
        name: 'Shimmer',
        symbol: 'SMR',
        decimals: 18,
      },
      rpcUrls: [
        'https://json-rpc.evm.shimmer.network',
        'wss://ws.json-rpc.evm.shimmer.network',
      ],
      blockExplorerUrls: ['https://explorer.evm.shimmer.network/'],
    },
    evmCustom: {
      chainAddress:
        'smr1prxvwqvwf7nru5q5xvh5thwg54zsm2y4wfnk6yk56hj3exxkg92mx20wl3s',
      aliasId:
        '0xccc7018e4fa63e5014332f45ddc8a5450da89572676d12d4d5e51c98d64155b3',
      toolkit: {
        url: 'https://evm-toolkit.evm.shimmer.network',
        hasFaucet: false,
      },
      api: 'https://api.evm.shimmer.network',
    },
  },
  shimmer_testnet: {
    baseToken: 'Testnet Token (no value)',
    protocol: 'Stardust',
    httpRestApi: 'https://api.testnet.shimmer.network',
    eventApi: 'wss://api.testnet.shimmer.network:443/api/mqtt/v1 (MQTT 3.1)',
    permaNodeApi: 'https://chronicle.testnet.shimmer.network',
    faucet: 'https://faucet.testnet.shimmer.network',
    explorer: 'https://explorer.shimmer.network/shimmer-testnet',
    evm: {
      chainId: '0x431',
      chainName: 'ShimmerEVM Testnet',
      nativeCurrency: {
        name: 'Shimmer',
        symbol: 'SMR',
        decimals: 18,
      },
      rpcUrls: ['https://json-rpc.evm.testnet.shimmer.network'],
      blockExplorerUrls: ['https://explorer.evm.testnet.shimmer.network/'],
    },
    evmCustom: {
      chainAddress:
        'rms1ppp00k5mmd2m8my8ukkp58nd3rskw6rx8l09aj35984k74uuc5u2cywn3ex',
      aliasId:
        '0x42f7da9bdb55b3ec87e5ac1a1e6d88e16768663fde5eca3429eb6f579cc538ac',
      toolkit: {
        url: 'https://evm-toolkit.evm.testnet.shimmer.network',
        hasFaucet: true,
      },
      api: 'https://api.evm.testnet.shimmer.network',
    },
  },
  iota_move: {
    baseToken: 'IOTA Token',
    jsonRpcUrl: 'jsonRpcUrl placeholder',
    jsonRpcWebsocketUrl:'jsonRpcWebsocketUrl placeholder',
    indexerRpc: 'indexerRpc placeholder',
    graphqlRpc: 'graphqlRpc placeholder',
    faucetUrl: 'faucetUrl placeholder',
    explorerUrl: 'https://explorer.rebased.iota.org'
  },
  iota_move_testnet: {
    baseToken: 'IOTA Token (no value)',
    jsonRpcUrl: 'https://api.testnet.iota.cafe',
    jsonRpcWebsocketUrl:'wss://api.testnet.iota.cafe',
    indexerRpc: 'https://indexer.testnet.iota.cafe',
    graphqlRpc: 'https://graphql.testnet.iota.cafe',
    faucetUrl: 'https://faucet.testnet.iota.cafe',
    explorerUrl: 'https://explorer.rebased.iota.org/?network=testnet'
  },
  iota_move_devnet: {
    baseToken: 'IOTA Token (no value)',
    jsonRpcUrl: 'https://api.devnet.iota.cafe',
    jsonRpcWebsocketUrl:'wss://api.devnet.iota.cafe',
    indexerRpc: 'https://indexer.devnet.iota.cafe',
    graphqlRpc: 'https://graphql.devnet.iota.cafe',
    faucetUrl: 'https://faucet.devnet.iota.cafe',
    explorerUrl: 'https://explorer.rebased.iota.org/?network=devnet'
  },
  iota_localnet: {
    baseToken:"IOTA Token", 
    jsonRpcUrl: 'http://127.0.0.1:9000',
    jsonRpcWebsocketUrl:'ws://127.0.0.1:9000',
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
    toolkit?: Toolkit;
    api?: string;
  };
}

export interface  MoveProps {
  jsonRpcUrl: string;
  jsonRpcWebsocketUrl: string;
  indexerRpc: string;
  graphqlRpc: string;
  faucetUrl: string;
  explorerUrl?: string;
};