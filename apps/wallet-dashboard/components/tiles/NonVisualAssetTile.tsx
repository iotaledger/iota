// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Card, CardAction, CardActionType, CardBody, CardType } from '@iota/apps-ui-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { formatAddress, parseStructTag } from '@iota/iota-sdk/utils';
import { ArrowTopRight } from '@iota/ui-icons';

type NonVisualAssetCardProps = {
    asset: IotaObjectData;
} & Pick<React.ComponentProps<typeof Card>, 'onClick'>;

export function NonVisualAssetCard({ asset, onClick }: NonVisualAssetCardProps): React.JSX.Element {
    const { address, module, name } = parseStructTag(asset.type!);
    return (
        <Card type={CardType.Default} isHoverable onClick={onClick}>
            <CardBody
                title={formatAddress(asset.objectId!)}
                subtitle={`${formatAddress(address)}::${module}::${name}`}
                isTextTruncated
            />
            <CardAction type={CardActionType.Link} icon={<ArrowTopRight />} />
        </Card>
    );
}
