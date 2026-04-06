// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Configuration } from '@rspack/core';

const configProd: Configuration = {
    extends: './configs/rspack/rspack.config.common.ts',
    mode: 'production',
    devtool: 'source-map',
};

export default configProd;
