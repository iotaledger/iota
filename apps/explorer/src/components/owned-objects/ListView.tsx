// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaObjectResponse } from '@iota/iota-sdk/client';
import { formatAddress } from '@iota/iota-sdk/utils';
import { Placeholder } from '@iota/ui';
import { type ReactNode } from 'react';
import {
    Table,
    TableCell,
    TableBodyRow,
    TableCellType,
    TableHeader,
    TableHeaderCell,
    TableHeaderRow,
} from '@iota/apps-ui-kit';
import cx from 'clsx';
import { ObjectLink, ObjectVideoImage } from '~/components/ui';
import { useResolveVideo } from '~/hooks/useResolveVideo';
import { parseObjectType, trimStdLibPrefix } from '~/lib/utils';

interface ListViewItemProps {
    assetCell?: ReactNode;
    typeCell?: ReactNode;
    objectIdCell?: ReactNode;
    objectId: string;
    loading?: boolean;
}

function ListViewItem({
    assetCell,
    typeCell,
    objectIdCell,
    objectId,
    loading,
}: ListViewItemProps): JSX.Element {
    const listViewItemContent = (
        <div
            className={cx(
                'flex items-center justify-around',
                '[&_td]:flex [&_td]:items-center',
                loading && 'group mb-2 justify-between rounded-lg p-1 hover:bg-hero/5',
            )}
        >
            <div
                className={cx(
                    'w-3/12 basis-3/12',
                    '[&_td]:flex [&_td]:items-center',
                    loading &&
                        'flex max-w-[66%] basis-8/12 items-center gap-3 md:max-w-[25%] md:basis-3/12 md:pr-5',
                )}
            >
                {loading ? <Placeholder rounded="lg" width="540px" height="20px" /> : assetCell}
            </div>

            <div
                className={cx(
                    'w-6/12 basis-6/12 [&_td]:flex',
                    loading && 'hidden max-w-[50%] pr-5 md:flex',
                )}
            >
                {loading ? <Placeholder rounded="lg" width="540px" height="20px" /> : typeCell}
            </div>

            <div
                className={cx(
                    'w-3/12 basis-3/12',
                    '[&_td]:flex [&_td]:items-center',
                    loading && 'flex max-w-[34%]',
                )}
            >
                {loading ? <Placeholder rounded="lg" width="540px" height="20px" /> : objectIdCell}
            </div>
        </div>
    );

    if (loading) {
        return listViewItemContent;
    }

    return <ObjectLink objectId={objectId} display="block" label={listViewItemContent} />;
}

function ListViewItemContainer({ obj }: { obj: IotaObjectResponse }): JSX.Element {
    const video = useResolveVideo(obj);
    const displayMeta = obj.data?.display?.data;
    const name = displayMeta?.name ?? displayMeta?.description ?? '';
    const type = trimStdLibPrefix(parseObjectType(obj));
    const objectId = obj.data?.objectId;

    return (
        <ListViewItem
            objectId={objectId!}
            assetCell={
                <TableCell
                    type={TableCellType.AvatarText}
                    leadingElement={
                        <ObjectVideoImage
                            fadeIn
                            disablePreview
                            title={name}
                            subtitle={type}
                            src={displayMeta?.image_url || ''}
                            video={video}
                            variant="xs"
                        />
                    }
                    label={name ? name : '--'}
                />
            }
            typeCell={<TableCell type={TableCellType.Text} label={type} />}
            objectIdCell={<TableCell type={TableCellType.Text} label={formatAddress(objectId!)} />}
        />
    );
}

interface ListViewProps {
    data?: IotaObjectResponse[];
    loading?: boolean;
}

export function ListView({ data, loading }: ListViewProps): JSX.Element {
    return (
        <div className="flex flex-col overflow-auto">
            <Table rowIndexes={data?.map((obj, index) => index) ?? []}>
                {(!!data?.length || loading) && (
                    <TableHeader>
                        <TableHeaderRow>
                            <div className="flex">
                                <div className="w-3/12 basis-3/12 [&_th]:flex [&_th]:items-center">
                                    <TableHeaderCell columnKey="assets" label="ASSETS" />
                                </div>
                                <div className="w-6/12 basis-6/12 [&_th]:flex [&_th]:items-center">
                                    <TableHeaderCell columnKey="type" label="TYPE" />
                                </div>
                                <div className="w-3/12 basis-3/12 [&_th]:flex [&_th]:items-center">
                                    <TableHeaderCell columnKey="objectId" label="OBJECT ID" />
                                </div>
                            </div>
                        </TableHeaderRow>
                    </TableHeader>
                )}
            </Table>

            {loading &&
                new Array(10)
                    .fill(0)
                    .map((_, index) => (
                        <ListViewItem key={index} objectId={String(index)} loading />
                    ))}

            <div className="flex h-full w-full flex-col overflow-auto">
                {data?.map((obj, index) => {
                    if (!obj.data) {
                        return null;
                    }
                    return (
                        <TableBodyRow key={obj.data.objectId} rowIndex={index}>
                            <ListViewItemContainer obj={obj} />
                        </TableBodyRow>
                    );
                })}
            </div>
        </div>
    );
}
