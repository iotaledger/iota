// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { AppDispatch } from '_store';
import { useDispatch } from 'react-redux';

export function useAppDispatch() {
    return useDispatch<AppDispatch>();
}
