// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DEFAULT_API_ENV } from '_app/ApiProvider';
import { API_ENV, type NetworkEnvType } from '_src/shared/api-env';
import { isValidUrl } from '_src/shared/utils';
import mitt from 'mitt';
import Browser from 'webextension-polyfill';

class NetworkEnv {
    #events = mitt<{ changed: NetworkEnvType }>();

    async getActiveNetwork(): Promise<NetworkEnvType> {
        const { iota_Env, iota_Env_RPC } = await Browser.storage.local.get({
            iota_Env: DEFAULT_API_ENV,
            iota_Env_RPC: null,
        });
        const adjCustomUrl = iota_Env === API_ENV.customRPC ? iota_Env_RPC : null;
        return { env: iota_Env, customRpcUrl: adjCustomUrl };
    }

    async setActiveNetwork(network: NetworkEnvType) {
        const { env, customRpcUrl } = network;
        if (env === API_ENV.customRPC && !isValidUrl(customRpcUrl)) {
            throw new Error(`Invalid custom RPC url ${customRpcUrl}`);
        }
        await Browser.storage.local.set({
            iota_Env: env,
            iota_Env_RPC: customRpcUrl,
        });
        this.#events.emit('changed', network);
    }

    on = this.#events.on;

    off = this.#events.off;
}

const networkEnv = new NetworkEnv();
export default networkEnv;
