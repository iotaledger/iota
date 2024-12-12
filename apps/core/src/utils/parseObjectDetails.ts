// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaObjectChangeWithDisplay } from '..';
import { IotaObjectChange } from '@iota/iota-sdk/client';

type ObjectChangeWithObjectType = Extract<
    IotaObjectChange | IotaObjectChangeWithDisplay,
    { objectType: string }
>;

type PackageId = string;
type ModuleName = string;
type TypeName = string;
export function parseObjectChangeDetails(
    objectChange: ObjectChangeWithObjectType,
): [PackageId, ModuleName, TypeName] {
    const [packageId, moduleName, typeName] = extractObjectTypeStruct(objectChange.objectType);
    return [packageId, moduleName, typeName];
}

export function extractObjectTypeStruct(objectType: string): [PackageId, ModuleName, TypeName] {
    const [packageId, moduleName, functionName] = objectType?.split('<')[0]?.split('::') || [];
    return [packageId, moduleName, functionName];
}
