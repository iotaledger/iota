// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type DisplayFieldsResponse } from '@iota/iota-sdk/client';
import { useState } from 'react';
import { Image, ObjectLink, ObjectModal } from '~/components/ui';

interface ObjectDisplayProps {
    objectId: string;
    display: DisplayFieldsResponse;
}

export function ObjectDisplay({ objectId, display }: ObjectDisplayProps): JSX.Element | null {
    const [open, handleOpen] = useState(false);
    if (!display.data) return null;
    const { description, name, image_url: imageUrl } = display.data ?? {};
    return (
        <div className="flex flex-row">
            <ObjectModal
                open={open}
                onClose={() => handleOpen(false)}
                title={name ?? description ?? ''}
                subtitle={description ?? ''}
                src={imageUrl ?? ''}
                alt={description ?? ''}
            />
            <div className="flex flex-row items-center gap-md">
                <div className="cursor-pointer">
                    <Image
                        size="sm"
                        rounded="2xl"
                        src={imageUrl ?? ''}
                        alt={description}
                        onClick={() => handleOpen(true)}
                    />
                </div>
                <div className="flex flex-col">
                    <span className="text-body-sm text-neutral-40 dark:text-neutral-60">
                        Object ID
                    </span>
                    <ObjectLink objectId={objectId} />
                </div>
            </div>
        </div>
    );
}
