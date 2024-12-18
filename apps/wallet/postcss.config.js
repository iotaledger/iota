// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const postcssPresetEnv = require('postcss-preset-env');
const tailwind = require('tailwindcss');

module.exports = {
    plugins: [postcssPresetEnv(), tailwind],
};
