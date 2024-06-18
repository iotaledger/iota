// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useContext } from 'react';
import { HiddenAssetsContext } from '../contexts';

export function useHiddenAssets() {
    return useContext(HiddenAssetsContext);
}
