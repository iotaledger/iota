// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createMessage } from '_messages';
import { WindowMessageStream } from '_messaging/WindowMessageStream';
import type { BasePayload, Payload } from '_payloads';
import type { GetAccount } from '_payloads/account/GetAccount';
import type { GetAccountResponse } from '_payloads/account/GetAccountResponse';
import type { SetNetworkPayload } from '_payloads/network';
import {
    ALL_PERMISSION_TYPES,
    type AcquirePermissionsRequest,
    type AcquirePermissionsResponse,
    type HasPermissionsRequest,
    type HasPermissionsResponse,
} from '_payloads/permissions';
import type {
    ExecuteTransactionRequest,
    ExecuteTransactionResponse,
    SignTransactionRequest,
    SignTransactionResponse,
} from '_payloads/transactions';
import { getCustomNetwork, type NetworkEnvType } from '_src/shared/api-env';
import { type SignMessageRequest } from '_src/shared/messaging/messages/payloads/transactions/SignMessage';
import { isWalletStatusChangePayload } from '_src/shared/messaging/messages/payloads/wallet-status-change';
import { getNetwork, Network, type ChainType } from '@iota/iota-sdk/client';
import { isTransactionBlock } from '@iota/iota-sdk/transactions';
import { fromB64, toB64 } from '@iota/iota-sdk/utils';
import {
    ReadonlyWalletAccount,
    SUPPORTED_CHAINS,
    type StandardConnectFeature,
    type StandardConnectMethod,
    type StandardEventsFeature,
    type StandardEventsListeners,
    type StandardEventsOnMethod,
    type IotaFeatures,
    type IotaSignAndExecuteTransactionBlockMethod,
    type IotaSignMessageMethod,
    type IotaSignPersonalMessageMethod,
    type IotaSignTransactionBlockMethod,
    type Wallet,
} from '@iota/wallet-standard';
import mitt, { type Emitter } from 'mitt';
import { filter, map, type Observable } from 'rxjs';

import { mapToPromise } from './utils';

type WalletEventsMap = {
    [E in keyof StandardEventsListeners]: Parameters<StandardEventsListeners[E]>[0];
};

// NOTE: Because this runs in a content script, we can't fetch the manifest.
const NAME = process.env.APP_NAME || 'IOTA Wallet';

export class IotaWallet implements Wallet {
    readonly #events: Emitter<WalletEventsMap>;
    readonly #version = '1.0.0' as const;
    readonly #name = NAME;
    #accounts: ReadonlyWalletAccount[];
    #messagesStream: WindowMessageStream;
    #activeChain: ChainType | null = null;

    get version() {
        return this.#version;
    }

    get name() {
        return this.#name;
    }

    get icon() {
        return 'data:image/png;base64,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...' as const;
    }

    get chains() {
        // TODO: Extract chain from wallet:
        return SUPPORTED_CHAINS;
    }

    get features(): StandardConnectFeature & StandardEventsFeature & IotaFeatures {
        return {
            'standard:connect': {
                version: '1.0.0',
                connect: this.#connect,
            },
            'standard:events': {
                version: '1.0.0',
                on: this.#on,
            },
            'iota:signTransactionBlock': {
                version: '1.0.0',
                signTransactionBlock: this.#signTransactionBlock,
            },
            'iota:signAndExecuteTransactionBlock': {
                version: '1.0.0',
                signAndExecuteTransactionBlock: this.#signAndExecuteTransactionBlock,
            },
            'iota:signMessage': {
                version: '1.0.0',
                signMessage: this.#signMessage,
            },
            'iota:signPersonalMessage': {
                version: '1.0.0',
                signPersonalMessage: this.#signPersonalMessage,
            },
        };
    }

    get accounts() {
        return this.#accounts;
    }

    #setAccounts(accounts: GetAccountResponse['accounts']) {
        this.#accounts = accounts.map(
            ({ address, publicKey, nickname }) =>
                new ReadonlyWalletAccount({
                    address,
                    label: nickname || undefined,
                    publicKey: publicKey ? fromB64(publicKey) : new Uint8Array(),
                    chains: this.#activeChain ? [this.#activeChain] : [],
                    features: ['iota:signAndExecuteTransaction'],
                }),
        );
    }

    constructor() {
        this.#events = mitt();
        this.#accounts = [];
        this.#messagesStream = new WindowMessageStream('iota_in-page', 'iota_content-script');
        this.#messagesStream.messages.subscribe(({ payload }) => {
            if (isWalletStatusChangePayload(payload)) {
                const { network, accounts } = payload;
                if (network) {
                    this.#setActiveChain(network);
                    if (!accounts) {
                        // in case an accounts change exists skip updating chains of current accounts
                        // accounts will be updated in the if block below
                        this.#accounts = this.#accounts.map(
                            ({ address, features, icon, label, publicKey }) =>
                                new ReadonlyWalletAccount({
                                    address,
                                    publicKey,
                                    chains: this.#activeChain ? [this.#activeChain] : [],
                                    features,
                                    label,
                                    icon,
                                }),
                        );
                    }
                }
                if (accounts) {
                    this.#setAccounts(accounts);
                }
                this.#events.emit('change', { accounts: this.accounts });
            }
        });
    }

    #on: StandardEventsOnMethod = (event, listener) => {
        this.#events.on(event, listener);
        return () => this.#events.off(event, listener);
    };

    #connected = async () => {
        this.#setActiveChain(await this.#getActiveNetwork());
        if (!(await this.#hasPermissions(['viewAccount']))) {
            return;
        }
        const accounts = await this.#getAccounts();
        this.#setAccounts(accounts);
        if (this.#accounts.length) {
            this.#events.emit('change', { accounts: this.accounts });
        }
    };

    #connect: StandardConnectMethod = async (input) => {
        if (!input?.silent) {
            await mapToPromise(
                this.#send<AcquirePermissionsRequest, AcquirePermissionsResponse>({
                    type: 'acquire-permissions-request',
                    permissions: ALL_PERMISSION_TYPES,
                }),
                (response) => response.result,
            );
        }

        await this.#connected();

        return { accounts: this.accounts };
    };

    #signTransactionBlock: IotaSignTransactionBlockMethod = async ({
        transactionBlock,
        account,
        ...input
    }) => {
        if (!isTransactionBlock(transactionBlock)) {
            throw new Error(
                'Unexpected transaction format found. Ensure that you are using the `Transaction` class.',
            );
        }

        return mapToPromise(
            this.#send<SignTransactionRequest, SignTransactionResponse>({
                type: 'sign-transaction-request',
                transaction: {
                    ...input,
                    // account might be undefined if previous version of adapters is used
                    // in that case use the first account address
                    account: account?.address || this.#accounts[0]?.address || '',
                    transaction: transactionBlock.serialize(),
                },
            }),
            (response) => response.result,
        );
    };

    #signAndExecuteTransactionBlock: IotaSignAndExecuteTransactionBlockMethod = async (input) => {
        if (!isTransactionBlock(input.transactionBlock)) {
            throw new Error(
                'Unexpected transaction format found. Ensure that you are using the `Transaction` class.',
            );
        }

        return mapToPromise(
            this.#send<ExecuteTransactionRequest, ExecuteTransactionResponse>({
                type: 'execute-transaction-request',
                transaction: {
                    type: 'transaction',
                    data: input.transactionBlock.serialize(),
                    options: input.options,
                    // account might be undefined if previous version of adapters is used
                    // in that case use the first account address
                    account: input.account?.address || this.#accounts[0]?.address || '',
                },
            }),
            (response) => response.result,
        );
    };

    #signMessage: IotaSignMessageMethod = async ({ message, account }) => {
        return mapToPromise(
            this.#send<SignMessageRequest, SignMessageRequest>({
                type: 'sign-message-request',
                args: {
                    message: toB64(message),
                    accountAddress: account.address,
                },
            }),
            (response) => {
                if (!response.return) {
                    throw new Error('Invalid sign message response');
                }
                return response.return;
            },
        );
    };

    #signPersonalMessage: IotaSignPersonalMessageMethod = async ({ message, account }) => {
        return mapToPromise(
            this.#send<SignMessageRequest, SignMessageRequest>({
                type: 'sign-message-request',
                args: {
                    message: toB64(message),
                    accountAddress: account.address,
                },
            }),
            (response) => {
                if (!response.return) {
                    throw new Error('Invalid sign message response');
                }
                return {
                    bytes: response.return.messageBytes,
                    signature: response.return.signature,
                };
            },
        );
    };

    #hasPermissions(permissions: HasPermissionsRequest['permissions']) {
        return mapToPromise(
            this.#send<HasPermissionsRequest, HasPermissionsResponse>({
                type: 'has-permissions-request',
                permissions: permissions,
            }),
            ({ result }) => result,
        );
    }

    #getAccounts() {
        return mapToPromise(
            this.#send<GetAccount, GetAccountResponse>({
                type: 'get-account',
            }),
            (response) => response.accounts,
        );
    }

    #getActiveNetwork() {
        return mapToPromise(
            this.#send<BasePayload, SetNetworkPayload>({
                type: 'get-network',
            }),
            ({ network }) => network,
        );
    }

    #setActiveChain({ network }: NetworkEnvType) {
        this.#activeChain =
            network === Network.Custom ? getCustomNetwork().chain : getNetwork(network).chain;
    }

    #send<RequestPayload extends Payload, ResponsePayload extends Payload | void = void>(
        payload: RequestPayload,
        responseForID?: string,
    ): Observable<ResponsePayload> {
        const msg = createMessage(payload, responseForID);
        this.#messagesStream.send(msg);
        return this.#messagesStream.messages.pipe(
            filter(({ id }) => id === msg.id),
            map((msg) => msg.payload as ResponsePayload),
        );
    }
}
