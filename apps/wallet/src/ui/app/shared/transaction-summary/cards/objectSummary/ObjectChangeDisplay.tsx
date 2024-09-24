// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExplorerLinkType } from '_components';
import { type IotaObjectChangeWithDisplay } from '@iota/core';

import { Card, CardAction, CardActionType, CardBody, CardImage, CardType } from '@iota/apps-ui-kit';
import { ImageIcon } from '../../../image-icon';
import { ArrowTopRight } from '@iota/ui-icons';
import { useExplorerLink } from '_src/ui/app/hooks/useExplorerLink';

export function ObjectChangeDisplay({ change }: { change: IotaObjectChangeWithDisplay }) {
    const display = change?.display?.data;
    const name = display?.name ?? '';
    const objectId = 'objectId' in change && change?.objectId;
    const explorerHref = useExplorerLink({
        type: ExplorerLinkType.Object,
        objectID: objectId?.toString() ?? '',
    });

    if (!display) return null;

    function handleOpen() {
        const newWindow = window.open(explorerHref!, '_blank', 'noopener,noreferrer');
        if (newWindow) newWindow.opener = null;
    }

    return (
        <Card type={CardType.Default} onClick={handleOpen}>
            <CardImage>
                <ImageIcon src={display.image_url ?? ''} label={name} fallback="NFT" />
            </CardImage>
            <CardBody title={name} subtitle={display.description ?? ''} />
            {objectId && <CardAction type={CardActionType.Link} icon={<ArrowTopRight />} />}
        </Card>
    );
}
