// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaObjectResponse } from '@iota/iota-sdk/client';
import { formatAddress } from '@iota/iota-sdk/utils';
import { Placeholder, Text } from '@iota/ui';
import { type ReactNode } from 'react';

import { OwnedObjectsText } from '~/components';
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
        <div className="group mb-2 flex items-center justify-between rounded-lg p-1 hover:bg-hero/5">
            <div className="flex max-w-[66%] basis-8/12 items-center gap-3 md:max-w-[25%] md:basis-3/12 md:pr-5">
                {loading ? <Placeholder rounded="lg" width="540px" height="20px" /> : assetCell}
            </div>

            <div className="hidden max-w-[50%] basis-6/12 pr-5 md:flex">
                {loading ? <Placeholder rounded="lg" width="540px" height="20px" /> : typeCell}
            </div>

            <div className="flex max-w-[34%] basis-3/12">
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
                <>
                    <ObjectVideoImage
                        fadeIn
                        disablePreview
                        title={name}
                        subtitle={type}
                        src={displayMeta?.image_url || ''}
                        video={video}
                        variant="xs"
                    />
                    <div className="flex flex-col overflow-hidden">
                        <OwnedObjectsText color="steel-darker" font="semibold">
                            {name || '--'}
                        </OwnedObjectsText>
                        <div className="block md:hidden">
                            <Text variant="pSubtitle/normal" color="steel-dark" truncate>
                                {type}
                            </Text>
                        </div>
                    </div>
                </>
            }
            typeCell={
                <OwnedObjectsText color="steel-dark" font="normal">
                    {type}
                </OwnedObjectsText>
            }
            objectIdCell={
                <OwnedObjectsText color="steel-dark" font="medium">
                    {formatAddress(objectId!)}
                </OwnedObjectsText>
            }
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
            {(!!data?.length || loading) && (
                <div className="mb-3.5 flex w-full justify-around">
                    <div className="max-w-[66%] basis-8/12 md:max-w-[25%] md:basis-3/12">
                        <Text variant="caption/medium" color="steel-dark">
                            ASSET
                        </Text>
                    </div>
                    <div className="hidden basis-6/12 md:block">
                        <Text variant="caption/medium" color="steel-dark">
                            TYPE
                        </Text>
                    </div>
                    <div className="basis-3/12">
                        <Text variant="caption/medium" color="steel-dark">
                            OBJECT ID
                        </Text>
                    </div>
                </div>
            )}
            {loading &&
                new Array(10)
                    .fill(0)
                    .map((_, index) => (
                        <ListViewItem key={index} objectId={String(index)} loading />
                    ))}

            <div className="flex h-full w-full flex-col overflow-auto">
                {data?.map((obj) => {
                    if (!obj.data) {
                        return null;
                    }
                    return <ListViewItemContainer key={obj.data.objectId} obj={obj} />;
                })}
            </div>
        </div>
    );
}
