// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TableCellBase, TableCellText } from '@iota/apps-ui-kit';
import type { ColumnDef } from '@tanstack/react-table';

import type { IotaObjectResponse } from '@iota/iota-sdk/client';
import { formatAddress, formatType } from '@iota/iota-sdk/utils';
import { ObjectLink, ObjectVideoImage } from '~/components';
import { useResolveVideo } from '~/hooks';
import { parseObjectType, trimStdLibPrefix } from '~/lib';

function Asset({ object }: { object: IotaObjectResponse }) {
    const video = useResolveVideo(object);
    const displayMeta = object.data?.display?.data;
    const name = displayMeta?.name ?? displayMeta?.description ?? '';
    const type = trimStdLibPrefix(parseObjectType(object));
    return (
        <div className="flex items-center gap-x-2.5 text-neutral-40 dark:text-neutral-60">
            <ObjectVideoImage
                fadeIn
                disablePreview
                title={name}
                subtitle={type}
                src={displayMeta?.image_url || ''}
                video={video}
                variant="xxs"
            />
            <div className="whitespace-nowrap text-label-lg">{name ? name : '--'}</div>
        </div>
    );
}

export function generateObjectListColumns(): ColumnDef<IotaObjectResponse>[] {
    return [
        {
            header: 'ASSETS',
            id: 'assets',
            cell({ row: { original: object } }) {
                const objectId = object?.data?.objectId;
                if (!objectId) return null;
                return (
                    <TableCellBase>
                        <ObjectLink
                            objectId={objectId}
                            display="block"
                            label={<Asset object={object} />}
                        />
                    </TableCellBase>
                );
            },
        },
        {
            header: 'TYPE',
            id: 'type',
            cell({ row: { original: object } }) {
                const objectId = object?.data?.objectId;
                if (!objectId) return null;
                const type = trimStdLibPrefix(formatType(parseObjectType(object)));
                return (
                    <TableCellBase>
                        <ObjectLink objectId={objectId} label={type}>
                            <TableCellText>{type}</TableCellText>
                        </ObjectLink>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'OBJECT ID',
            id: 'objectId',
            cell({ row: { original: object } }) {
                const objectId = object?.data?.objectId;
                if (!objectId) return null;
                const address = formatAddress(objectId);
                return (
                    <TableCellBase>
                        <ObjectLink
                            objectId={objectId}
                            label={
                                <TableCellText>
                                    <div className="whitespace-nowrap">{address}</div>
                                </TableCellText>
                            }
                        />
                    </TableCellBase>
                );
            },
        },
    ];
}
