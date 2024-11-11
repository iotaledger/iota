// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { hasDisplayData } from '@iota/core';
import { PaginatedObjectsResponse } from '@iota/iota-sdk/client';

export function filterAssetList(pages: PaginatedObjectsResponse[], shouldBeVisual: boolean) {
    return (
        pages
            .flatMap((page) => page.data)
            .filter(
                (asset) =>
                    asset.data && asset.data.objectId && hasDisplayData(asset) === shouldBeVisual,
            )
            .map((response) => response.data!) ?? []
    );
}
