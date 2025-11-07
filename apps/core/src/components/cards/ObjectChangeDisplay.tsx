// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaObjectChangeWithDisplay, ExplorerLinkType, NFTMediaRenderer } from '../../';
import { Card, CardAction, CardActionType, CardBody, CardImage, CardType } from '@iota/apps-ui-kit';
import { ArrowTopRight } from '@iota/apps-ui-icons';
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
                    <NFTMediaRenderer
                        src={display.image_url ?? ''}
                        disableVideoControls
                        disableAutoPlay
                    />
                </CardImage>
                <div className="truncate overflow-x-hidden [&_div]:truncate">
                    <CardBody title={name} subtitle={display.description ?? ''} />
                </div>
                {objectId && <CardAction type={CardActionType.Link} icon={<ArrowTopRight />} />}
            </Card>
        </ExplorerLink>
    );
}
