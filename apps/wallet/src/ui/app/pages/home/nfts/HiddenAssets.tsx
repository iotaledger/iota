// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { HiddenAsset, type HiddenAssetProps } from './HiddenAsset';

interface HiddenAssetsProps {
    items: HiddenAssetProps[];
}

export function HiddenAssets({ items }: HiddenAssetsProps) {
    return (
        <div className="flex w-full flex-col overflow-y-auto">
            {items?.map((object) => <HiddenAsset key={object.data!.objectId} {...object} />)}
        </div>
    );
}
