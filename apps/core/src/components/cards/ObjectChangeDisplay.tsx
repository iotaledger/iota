// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaObjectChangeWithDisplay, ExplorerLinkType, ImageIcon } from '../../';
import { Card, CardAction, CardActionType, CardBody, CardImage, CardType } from '@iota/apps-ui-kit';
import { ArrowTopRight } from '@iota/ui-icons';
import { RenderExplorerLink } from '../../types';

interface ObjectChangeDisplayProps {
    renderExplorerLink: RenderExplorerLink;
    change?: IotaObjectChangeWithDisplay;
}
export function ObjectChangeDisplay({
    change,
    renderExplorerLink: ExplorerLink,
}: ObjectChangeDisplayProps) {
    const display = change?.display?.data;
    const name = display?.name ?? '';
    const objectId = change && 'objectId' in change && change?.objectId;

    if (!display) return null;

    return (
        <ExplorerLink objectID={objectId?.toString() ?? ''} type={ExplorerLinkType.Object}>
            <Card type={CardType.Default} isHoverable>
                <CardImage>
                    <ImageIcon src={display.image_url ?? ''} label={name} fallback="NFT" />
                </CardImage>
                <CardBody title={name} subtitle={display.description ?? ''} />
                {objectId && <CardAction type={CardActionType.Link} icon={<ArrowTopRight />} />}
            </Card>
        </ExplorerLink>
    );
}
