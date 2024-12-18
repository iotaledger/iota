// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import {
    DisplayFieldsResponse,
    IotaObjectChange,
    IotaObjectChangeCreated,
    IotaObjectChangeDeleted,
    IotaObjectChangeMutated,
    IotaObjectChangePublished,
    IotaObjectChangeTransferred,
    IotaObjectChangeWrapped,
} from '@iota/iota-sdk/client';

import { groupByOwner } from './groupByOwner';
import { IotaObjectChangeTypes } from './types';

export type WithDisplayFields<T> = T & { display?: DisplayFieldsResponse };
export type IotaObjectChangeWithDisplay = WithDisplayFields<IotaObjectChange>;

export type ObjectChanges = {
    changesWithDisplay: IotaObjectChangeWithDisplay[];
    changes: IotaObjectChange[];
    ownerType: string;
};
export type ObjectChangesByOwner = Record<string, ObjectChanges>;

export type ObjectChangeSummary = {
    [K in IotaObjectChangeTypes]: ObjectChangesByOwner;
};

export const getObjectChangeSummary = (objectChanges: IotaObjectChangeWithDisplay[]) => {
    if (!objectChanges) return null;

    const mutated = objectChanges.filter(
        (change) => change.type === 'mutated',
    ) as IotaObjectChangeMutated[];

    const created = objectChanges.filter(
        (change) => change.type === 'created',
    ) as IotaObjectChangeCreated[];

    const transferred = objectChanges.filter(
        (change) => change.type === 'transferred',
    ) as IotaObjectChangeTransferred[];

    const published = objectChanges.filter(
        (change) => change.type === 'published',
    ) as IotaObjectChangePublished[];

    const wrapped = objectChanges.filter(
        (change) => change.type === 'wrapped',
    ) as IotaObjectChangeWrapped[];

    const deleted = objectChanges.filter(
        (change) => change.type === 'deleted',
    ) as IotaObjectChangeDeleted[];

    return {
        transferred: groupByOwner(transferred),
        created: groupByOwner(created),
        mutated: groupByOwner(mutated),
        published: groupByOwner(published),
        wrapped: groupByOwner(wrapped),
        deleted: groupByOwner(deleted),
    };
};
