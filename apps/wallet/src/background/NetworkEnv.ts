// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type NetworkEnvType } from '@iota/core';
import { isValidUrl } from '_src/shared/utils';
import { getDefaultNetwork, Network } from '@iota/iota-sdk/client';
import mitt from 'mitt';
import Browser from 'webextension-polyfill';

class NetworkEnv {
    #events = mitt<{ changed: NetworkEnvType }>();

    async getActiveNetwork(): Promise<NetworkEnvType> {
        const { network, customRpc } = await Browser.storage.local.get({
            network: getDefaultNetwork(),
            customRpc: null,
        });
        const adjCustomUrl = network === Network.Custom ? customRpc : null;
        return { network, customRpcUrl: adjCustomUrl };
    }

    async setActiveNetwork(networkEnv: NetworkEnvType) {
        const { network, customRpcUrl } = networkEnv;
        if (network === Network.Custom && !isValidUrl(customRpcUrl)) {
            throw new Error(`Invalid custom RPC url ${customRpcUrl}`);
        }
        await Browser.storage.local.set({
            network,
            customRpc: customRpcUrl,
        });
        this.#events.emit('changed', networkEnv);
    }

    on = this.#events.on;

    off = this.#events.off;
}

const networkEnv = new NetworkEnv();
export default networkEnv;
