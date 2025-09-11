// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import { useEffect } from 'react';

export function useInitialPageView() {
    useEffect(() => {
        ampli.identify(undefined);
    }, []);
}
