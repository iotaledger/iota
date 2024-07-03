// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import ESLintPlugin from 'eslint-rspack-plugin';
import { merge } from 'webpack-merge';

import configCommon from './rspack.config.common';

const configDev = {
    mode: 'development',
    devtool: 'cheap-source-map',
    plugins: [new ESLintPlugin({ extensions: ['ts', 'tsx', 'js', 'jsx'] })],
    watchOptions: {
        aggregateTimeout: 600,
    },
    stats: {
        loggingDebug: ['sass-loader'],
    },
};

async function getConfig() {
    return merge(await configCommon(), configDev);
}

export default getConfig;
