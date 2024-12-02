// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DisplayFieldsResponse, IotaObjectChange } from '@iota/iota-sdk/client';

export type IotaObjectChangeTypes =
    | 'published'
    | 'transferred'
    | 'mutated'
    | 'deleted'
    | 'wrapped'
    | 'created';

export type WithDisplayFields<T> = T & { display?: DisplayFieldsResponse };
export type IotaObjectChangeWithDisplay = WithDisplayFields<IotaObjectChange>;

export type ObjectChange = {
    changesWithDisplay: IotaObjectChangeWithDisplay[];
    changes: IotaObjectChange[];
    ownerType: string;
};
export type ObjectChangesByOwner = Record<string, ObjectChange>;

export type ObjectChangeSummary = {
    [K in IotaObjectChangeTypes]: ObjectChangesByOwner;
};
