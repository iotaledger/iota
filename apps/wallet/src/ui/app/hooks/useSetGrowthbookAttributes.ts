// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAppSelector } from '_hooks';
import { setAttributes } from '_src/shared/experimentation/features';
import { useGrowthBook } from '@growthbook/growthbook-react';
import { useEffect } from 'react';

export function useSetGrowthbookAttributes() {
    const { network, customRpc } = useAppSelector((state) => state.app);
    const growthBook = useGrowthBook();

    useEffect(() => {
        if (growthBook) {
            setAttributes({ network, customRpc });
        }
    }, [growthBook, network, customRpc]);
}
