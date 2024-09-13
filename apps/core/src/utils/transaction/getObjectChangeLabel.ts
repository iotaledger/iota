// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaObjectChangeTypes } from './types';

export const ObjectChangeLabels = {
    created: 'Created',
    mutated: 'Updated',
    transferred: 'Transfer',
    published: 'Publish',
    deleted: 'Deleted',
    wrapped: 'Wrap',
};

export function getObjectChangeLabel(type: IotaObjectChangeTypes) {
    return ObjectChangeLabels[type];
}
