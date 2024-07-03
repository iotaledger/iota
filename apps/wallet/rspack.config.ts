// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import configDev from './configs/webpack/rspack.config.dev';
import configProd from './configs/webpack/rspack.config.prod';

const configMap: Record<string, () => Promise<any>> = {
    development: configDev,
    production: configProd,
};

const nodeEnv: string = process.env.NODE_ENV || '';

if (!configMap[nodeEnv]) {
    throw new Error(`Config not found for NODE_ENV='${nodeEnv}'`);
}

export default configMap[nodeEnv];
