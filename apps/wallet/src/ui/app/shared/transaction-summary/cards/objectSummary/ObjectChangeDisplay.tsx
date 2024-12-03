// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExplorerLink, ExplorerLinkType } from '_components';
import { type IotaObjectChangeWithDisplay, ImageIcon } from '@iota/core';

import { Card, CardAction, CardActionType, CardBody, CardImage, CardType } from '@iota/apps-ui-kit';
import { ArrowTopRight } from '@iota/ui-icons';

export function ObjectChangeDisplay({ change }: { change: IotaObjectChangeWithDisplay }) {
    const display = change?.display?.data;
    const name = display?.name ?? '';
    const objectId = 'objectId' in change && change?.objectId;

    if (!display) return null;

    return (
        <ExplorerLink
            className="text-hero-dark no-underline"
            objectID={objectId?.toString() ?? ''}
            type={ExplorerLinkType.Object}
        >
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
