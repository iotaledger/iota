// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type Configuration, DefinePlugin } from 'webpack';
import { merge } from 'webpack-merge';

import configCommon from './webpack.config.common';

const configProd: Configuration = {
    mode: 'production',
    devtool: 'source-map',
    plugins: [new DefinePlugin({
        'process.env.WALLET_RC_VERSION': JSON.stringify(process.env.WALLET_RC_VERSION || process.env.WALLET_RC_VERSION)
      })],
};

async function getConfig(env: any) {
    console.log("env ---------------- ", env);
    return merge(await configCommon(), configProd);
}

export default getConfig;
