// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Configuration } from 'webpack';

import configDev from './configs/webpack/webpack.config.dev';
import configProd from './configs/webpack/webpack.config.prod';

const configMap: Record<string, (env: any) => Promise<Configuration>> = {
    development: configDev,
    production: configProd,
};

const nodeEnv: string = process.env.NODE_ENV || '';

console.log('WALLET_RC_VERSION:', process.env.WALLET_RC_VERSION);
console.log('configMap:', configMap);

if (!configMap[nodeEnv]) {
    throw new Error(`Config not found for NODE_ENV='${nodeEnv}'`);
}

export default configMap[nodeEnv];
