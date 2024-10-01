// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import ESLintPlugin from 'eslint-webpack-plugin';
import { type Configuration, DefinePlugin } from 'webpack';
import { merge } from 'webpack-merge';

import configCommon from './webpack.config.common';

const configDev: Configuration = {
    mode: 'development',
    devtool: 'cheap-source-map',
    plugins: [new ESLintPlugin({ extensions: ['ts', 'tsx', 'js', 'jsx'] }), new DefinePlugin({
        'process.env.WALLET_RC_VERSION': JSON.stringify(process.env.WALLET_RC_VERSION || process.env.WALLET_RC_VERSION)
      })],
    watchOptions: {
        aggregateTimeout: 600,
    },
    stats: {
        loggingDebug: ['sass-loader'],
    },
};

async function getConfig(env: any) {
    console.log("env ---------------- ", env);
    
    return merge(await configCommon(), configDev);
}

export default getConfig;
