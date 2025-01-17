// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Card, CardAction, CardActionType, CardBody, CardImage, CardType } from '@iota/apps-ui-kit';
import { ImageIcon } from '@iota/core';
import { type DisplayFieldsResponse } from '@iota/iota-sdk/client';
import { ArrowTopRight } from '@iota/apps-ui-icons';
import { useState } from 'react';
import { ObjectModal } from '~/components/ui';

interface ObjectDisplayProps {
    objectId: string;
    display: DisplayFieldsResponse;
}

export function ObjectDisplay({ objectId, display }: ObjectDisplayProps): JSX.Element | null {
    const [open, handleOpenModal] = useState(false);
    if (!display.data) return null;
    const { description, name, image_url: imageUrl } = display.data ?? {};

    function handleOpen() {
        const newWindow = window.open(`/object/${objectId}`, '_blank', 'noopener,noreferrer');
        if (newWindow) newWindow.opener = null;
    }
    return (
        <div className="flex w-full flex-row">
            <ObjectModal
                open={open}
                onClose={() => handleOpenModal(false)}
                title={name ?? description ?? ''}
                subtitle={description ?? ''}
                src={imageUrl ?? ''}
                alt={description ?? ''}
            />
            <Card type={CardType.Default} onClick={() => handleOpenModal(true)}>
                <CardImage>
                    <ImageIcon src={imageUrl ?? ''} label={name} fallback="NFT" />
                </CardImage>
                <CardBody title={name} subtitle={description ?? ''} />
                <CardAction
                    type={CardActionType.Link}
                    icon={<ArrowTopRight />}
                    onClick={handleOpen}
                />
            </Card>
        </div>
    );
}
