// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaObjectChangeWithDisplay } from '..';
import { IotaObjectChange } from '@iota/iota-sdk/client';

type ObjectChangeWithObjectType = Extract<
    IotaObjectChange | IotaObjectChangeWithDisplay,
    { objectType: string }
>;

export function parseObjectChangeDetails(
    objectChange: ObjectChangeWithObjectType,
): [string, string, string] {
    const [packageId, moduleName, typeName] =
        objectChange.objectType?.split('<')[0]?.split('::') || [];
    return [packageId, moduleName, typeName];
}
