// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Card, CardAction, CardActionType, CardBody, CardType } from '@iota/apps-ui-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { formatAddress, parseStructTag } from '@iota/iota-sdk/utils';
import { ArrowTopRight } from '@iota/apps-ui-icons';
import { ExplorerLink } from '../ExplorerLink';
import { ExplorerLinkType } from '@iota/core';

type NonVisualAssetCardProps = {
    asset: IotaObjectData;
} & React.ComponentProps<typeof Card>;

export function NonVisualAssetCard({ asset }: NonVisualAssetCardProps): React.JSX.Element {
    const { address, module, name } = parseStructTag(asset.type!);
    return (
        <ExplorerLink objectID={asset.objectId} type={ExplorerLinkType.Object}>
            <Card type={CardType.Default} isHoverable>
                <CardBody
                    title={formatAddress(asset.objectId!)}
                    subtitle={`${formatAddress(address)}::${module}::${name}`}
                    isTextTruncated
                />
                <CardAction type={CardActionType.Link} icon={<ArrowTopRight />} />
            </Card>
        </ExplorerLink>
    );
}
