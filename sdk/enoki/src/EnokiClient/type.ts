// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { PublicKey } from '@iota/iota.js/cryptography';
import type { ZkLoginSignatureInputs } from '@iota/iota.js/zklogin';

import type { AuthProvider } from '../EnokiFlow.js';

export type EnokiNetwork = 'mainnet' | 'testnet' | 'devnet';

export interface GetAppApiInput {}
export interface GetAppApiResponse {
    authenticationProviders: {
        providerType: AuthProvider;
        clientId: string;
    }[];
}

export interface GetZkLoginApiInput {
    jwt: string;
}
export interface GetZkLoginApiResponse {
    address: string;
    salt: string;
}

export interface CreateZkLoginNonceApiInput {
    network?: EnokiNetwork;
    ephemeralPublicKey: PublicKey;
    additionalEpochs?: number;
}
export interface CreateZkLoginNonceApiResponse {
    nonce: string;
    randomness: string;
    epoch: number;
    maxEpoch: number;
    estimatedExpiration: number;
}

export interface CreateZkLoginZkpApiInput {
    network?: EnokiNetwork;
    jwt: string;
    ephemeralPublicKey: PublicKey;
    randomness: string;
    maxEpoch: number;
}
export interface CreateZkLoginZkpApiResponse extends ZkLoginSignatureInputs {}

export type CreateSponsoredTransactionBlockApiInput = {
    network?: EnokiNetwork;
    transactionBlockKindBytes: string;
} & (
    | {
          jwt: string;
          sender?: never;
          allowedAddresses?: never;
          allowedMoveCallTargets?: never;
      }
    | {
          sender: string;
          allowedAddresses?: string[];
          allowedMoveCallTargets?: string[];
          jwt?: never;
      }
);

export interface CreateSponsoredTransactionBlockApiResponse {
    bytes: string;
    digest: string;
}

export interface ExecuteSponsoredTransactionBlockApiInput {
    digest: string;
    signature: string;
}

export interface ExecuteSponsoredTransactionBlockApiResponse {
    digest: string;
}
