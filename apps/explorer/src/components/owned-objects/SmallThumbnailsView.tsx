// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaObjectResponse } from '@iota/iota-sdk/client';
import { formatAddress } from '@iota/iota-sdk/utils';
import { Placeholder } from '@iota/ui';
import { type ReactNode } from 'react';

import { OwnedObjectsText } from '~/components';
import { ObjectLink, ObjectVideoImage } from '~/components/ui';
import { useResolveVideo } from '~/hooks/useResolveVideo';
import { parseObjectType, trimStdLibPrefix } from '~/lib/utils';

interface SmallThumbnailsViewProps {
    limit: number;
    data?: IotaObjectResponse[];
    loading?: boolean;
}

interface OwnObjectContainerProps {
    id: string;
    children: ReactNode;
}

function OwnObjectContainer({ id, children }: OwnObjectContainerProps): JSX.Element {
    return (
        <div className="w-full min-w-smallThumbNailsViewContainerMobile basis-1/2 pb-3 pr-4 md:min-w-smallThumbNailsViewContainer md:basis-1/4">
            <div className="rounded-lg p-2 hover:bg-hero/5">
                <ObjectLink display="block" objectId={id} label={children} />
            </div>
        </div>
    );
}

function SmallThumbnailsViewLoading({ limit }: { limit: number }): JSX.Element {
    return (
        <>
            {new Array(limit).fill(0).map((_, index) => (
                <OwnObjectContainer key={index} id={String(index)}>
                    <Placeholder rounded="lg" height="80px" />
                </OwnObjectContainer>
            ))}
        </>
    );
}

function SmallThumbnail({ obj }: { obj: IotaObjectResponse }): JSX.Element {
    const video = useResolveVideo(obj);
    const displayMeta = obj.data?.display?.data;
    const src = displayMeta?.image_url || '';
    const name = displayMeta?.name ?? displayMeta?.description ?? '--';
    const type = trimStdLibPrefix(parseObjectType(obj));
    const id = obj.data?.objectId;

    return (
        <div className="group flex items-center gap-3.75 overflow-auto">
            <ObjectVideoImage
                fadeIn
                disablePreview
                title={name}
                subtitle={type}
                src={src}
                video={video}
                variant="small"
            />

            <div className="flex min-w-0 flex-col flex-nowrap gap-1.25">
                <OwnedObjectsText color="steel-darker" font="semibold">
                    {name}
                </OwnedObjectsText>
                <OwnedObjectsText color="steel-dark" font="medium">
                    {formatAddress(id!)}
                </OwnedObjectsText>
            </div>
        </div>
    );
}

export function SmallThumbnailsView({
    data,
    loading,
    limit,
}: SmallThumbnailsViewProps): JSX.Element {
    return (
        <div className="flex flex-row flex-wrap overflow-auto">
            {loading && <SmallThumbnailsViewLoading limit={limit} />}
            {data?.map((obj, index) => {
                const id = obj.data?.objectId;

                return (
                    <OwnObjectContainer key={id} id={id!}>
                        <SmallThumbnail obj={obj} />
                    </OwnObjectContainer>
                );
            })}
        </div>
    );
}
