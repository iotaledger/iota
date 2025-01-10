// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { thunkExtras } from '../redux/store/thunkExtras';

export function useBackgroundClient() {
    return thunkExtras.background;
}
