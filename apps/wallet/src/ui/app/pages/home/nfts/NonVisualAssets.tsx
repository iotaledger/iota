// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExplorerLink, ExplorerLinkType } from '_components';
import { Text } from '_src/ui/app/shared/text';
import { type IotaObjectData } from '@iota/iota.js/client';
import { formatAddress, parseStructTag } from '@iota/iota.js/utils';

interface NonVisualAssetsProps {
    items: IotaObjectData[];
}

export default function NonVisualAssets({ items }: NonVisualAssetsProps) {
    return (
        <div className="flex w-full flex-1 flex-col items-center gap-4">
            {items?.length ? (
                <div className="divide-gray-40 flex w-full flex-col flex-wrap gap-3 divide-x-0 divide-y divide-solid">
                    {items.map((item) => {
                        const { address, module, name } = parseStructTag(item.type!);
                        return (
                            <div className="grid grid-cols-3 pt-3" key={item.objectId}>
                                <ExplorerLink
                                    className="text-hero-dark no-underline"
                                    objectID={item.objectId!}
                                    type={ExplorerLinkType.Object}
                                >
                                    <Text variant="pBody">{formatAddress(item.objectId!)}</Text>
                                </ExplorerLink>

                                <div className="col-span-2 break-all">
                                    <Text
                                        variant="pBodySmall"
                                        weight="normal"
                                        mono
                                        color="steel"
                                        title={item.type ?? ''}
                                    >
                                        {`${formatAddress(address)}::${module}::${name}`}
                                    </Text>
                                </div>
                            </div>
                        );
                    })}
                </div>
            ) : (
                <div className="text-steel-darker flex flex-1 items-center self-center text-caption font-semibold">
                    No Assets found
                </div>
            )}
        </div>
    );
}
