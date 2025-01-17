// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Tooltip, TooltipPosition } from '@iota/apps-ui-kit';
import { type IotaObjectResponse } from '@iota/iota-sdk/client';
import { formatAddress } from '@iota/iota-sdk/utils';
import { Info, Loader } from '@iota/apps-ui-icons';
import { type ReactNode } from 'react';
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
        <div className="rounded-xl p-xs hover:bg-neutral-92 dark:hover:bg-neutral-12">
            <ObjectLink display="block" objectId={id} label={children} />
        </div>
    );
}

function SmallThumbnailsViewLoading({ limit }: { limit: number }): JSX.Element {
    return (
        <>
            {new Array(limit).fill(0).map((_, index) => (
                <OwnObjectContainer key={index} id={String(index)}>
                    <Loader className="animate-spin" />
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
        <div className="flex items-center gap-md">
            <ObjectVideoImage
                fadeIn
                disablePreview
                title={name}
                subtitle={type}
                src={src}
                video={video}
                variant="xs"
            />
            <div className="flex min-w-0 flex-col flex-nowrap gap-xxs">
                <span className="text-label-md text-neutral-10 dark:text-neutral-92">{name}</span>
                <div className="flex flex-row items-center gap-xs text-label-md text-neutral-10 dark:text-neutral-92">
                    <span className="text-label-sm text-neutral-40 dark:text-neutral-60">
                        {formatAddress(id!)}
                    </span>
                    <Tooltip text={type} position={TooltipPosition.Bottom}>
                        <Info className="text-neutral-60 dark:text-neutral-40" />
                    </Tooltip>
                </div>
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
        <div className="h-full overflow-auto">
            <div className="grid grid-cols-[repeat(auto-fill,minmax(250px,1fr))] gap-y-xxs">
                {loading && <SmallThumbnailsViewLoading limit={limit} />}
                {data?.map((obj) => {
                    const id = obj.data?.objectId;

                    return (
                        <OwnObjectContainer key={id} id={id!}>
                            <SmallThumbnail obj={obj} />
                        </OwnObjectContainer>
                    );
                })}
            </div>
        </div>
    );
}
