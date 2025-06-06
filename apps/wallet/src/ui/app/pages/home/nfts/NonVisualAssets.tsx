// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExplorerLink, ExplorerLinkType } from '_components';
import { type IotaObjectData } from '@iota/iota-sdk/client';
import { formatAddress, parseStructTag } from '@iota/iota-sdk/utils';
import { Card, CardAction, CardActionType, CardBody, CardType } from '@iota/apps-ui-kit';
import { ArrowTopRight } from '@iota/apps-ui-icons';
import { NoData } from '@iota/core';

interface NonVisualAssetsProps {
    items: IotaObjectData[];
}

export function NonVisualAssets({ items }: NonVisualAssetsProps) {
    return (
        <div className="flex w-full flex-1 flex-col items-center gap-4">
            {items?.length ? (
                <div className="flex w-full flex-col">
                    {items.map((item) => {
                        const { address, module, name } = parseStructTag(item.type!);
                        return (
                            <ExplorerLink
                                className="text-hero-dark no-underline"
                                objectID={item.objectId!}
                                type={ExplorerLinkType.Object}
                                key={item.objectId}
                            >
                                <Card type={CardType.Default} isHoverable>
                                    <CardBody
                                        title={formatAddress(item.objectId!)}
                                        subtitle={`${formatAddress(address)}::${module}::${name}`}
                                        isTextTruncated
                                    />
                                    <CardAction
                                        type={CardActionType.Link}
                                        icon={<ArrowTopRight />}
                                    />
                                </Card>
                            </ExplorerLink>
                        );
                    })}
                </div>
            ) : (
                <NoData message="No non-visual assets found." />
            )}
        </div>
    );
}
