// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaObjectResponse } from '@iota/iota-sdk/client';

import { useResolveVideo } from '~/hooks/useResolveVideo';
import { parseObjectType, trimStdLibPrefix } from '~/lib/utils';
import { ObjectDetails } from '~/components/ui';

type OwnedObjectProps = {
    obj: IotaObjectResponse;
};

export default function OwnedObject({ obj }: OwnedObjectProps): JSX.Element {
    const video = useResolveVideo(obj);
    const displayMeta = obj.data?.display?.data;

    return (
        <ObjectDetails
            noTypeRender
            variant="small"
            id={obj.data?.objectId}
            type={trimStdLibPrefix(parseObjectType(obj))}
            name={displayMeta?.name ?? displayMeta?.description ?? '--'}
            image={displayMeta?.image_url}
            video={video}
        />
    );
}
