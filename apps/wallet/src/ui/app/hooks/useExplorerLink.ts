// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ExplorerLinkConfig, getExplorerLink as useGetExplorerLink } from '@iota/core';
import { useActiveAddress } from './useActiveAddress';
import { useAppSelector } from './useAppSelector';

export function useExplorerLink(linkConfig: ExplorerLinkConfig) {
    const app = useAppSelector(({ app }) => app);
    const activeAddress = useActiveAddress();
    const link = useGetExplorerLink(linkConfig, activeAddress, app.network);
    return link;
}
