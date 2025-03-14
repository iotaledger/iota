// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useRecognizedPackages } from '@iota/core';
import { type IotaObjectResponse } from '@iota/iota-sdk/client';

export function useResolveVideo(object?: IotaObjectResponse | null) {
    const recognizedPackages = useRecognizedPackages();

    if (!object) return null;

    const objectType =
        object.data?.type ??
        (object?.data?.content?.dataType === 'package' ? 'package' : object?.data?.content?.type) ??
        null;
    const isRecognized = objectType && recognizedPackages.includes(objectType.split('::')[0]);

    if (!isRecognized) return null;

    const display = object.data?.display?.data;

    return display?.video_url;
}
