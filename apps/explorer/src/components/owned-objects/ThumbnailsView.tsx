// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaObjectResponse } from '@iota/iota-sdk/client';
import { formatAddress } from '@iota/iota-sdk/utils';
import { Loader } from '@iota/apps-ui-icons';
import { ObjectLink, ObjectVideoImage } from '~/components/ui';
import { useResolveVideo } from '~/hooks/useResolveVideo';
import { parseObjectType, trimStdLibPrefix } from '~/lib/utils';

function Thumbnail({ obj }: { obj: IotaObjectResponse }): JSX.Element {
    const video = useResolveVideo(obj);
    const displayMeta = obj.data?.display?.data;
    const src = displayMeta?.image_url || '';
    const name = displayMeta?.name ?? displayMeta?.description;
    const type = trimStdLibPrefix(parseObjectType(obj));
    const id = obj.data?.objectId;
    const displayName = name || formatAddress(id!);

    return (
        <div>
            <ObjectLink
                display="flex"
                objectId={id!}
                label={
                    <div className="group relative">
                        <ObjectVideoImage
                            fadeIn
                            disablePreview
                            title={name || '--'}
                            subtitle={type}
                            src={src}
                            video={video}
                            variant="medium"
                        />
                        <div className="absolute bottom-0 flex h-full w-full items-end justify-start rounded-xl p-xs opacity-0 transition-opacity duration-300 group-hover:bg-shader-neutral-light-48 group-hover:opacity-100 group-hover:transition group-hover:duration-300 group-hover:ease-in-out group-hover:dark:bg-shader-primary-dark-48">
                            <span className="self-center text-label-md text-neutral-100">
                                {displayName}
                            </span>
                        </div>
                    </div>
                }
            />
        </div>
    );
}

function ThumbnailsOnlyLoading({ limit }: { limit: number }): JSX.Element {
    return (
        <>
            {new Array(limit).fill(0).map((_, index) => (
                <div key={index} className="md:h-31.5 md:w-31.5 h-16 w-16 text-primary-30">
                    <Loader className="animate-spin" />
                </div>
            ))}
        </>
    );
}

interface ThumbnailsViewViewProps {
    limit: number;
    data?: IotaObjectResponse[];
    loading?: boolean;
}

export function ThumbnailsView({ data, loading, limit }: ThumbnailsViewViewProps): JSX.Element {
    return (
        <div className="flex flex-row flex-wrap gap-2 overflow-auto md:gap-4">
            {loading ? (
                <ThumbnailsOnlyLoading limit={limit} />
            ) : (
                data?.map((obj) => <Thumbnail key={obj.data?.objectId} obj={obj} />)
            )}
        </div>
    );
}
