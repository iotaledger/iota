// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaObjectChange } from './generated.js';

export type IotaObjectChangePublished = Extract<IotaObjectChange, { type: 'published' }>;
export type IotaObjectChangeTransferred = Extract<IotaObjectChange, { type: 'transferred' }>;
export type IotaObjectChangeMutated = Extract<IotaObjectChange, { type: 'mutated' }>;
export type IotaObjectChangeDeleted = Extract<IotaObjectChange, { type: 'deleted' }>;
export type IotaObjectChangeWrapped = Extract<IotaObjectChange, { type: 'wrapped' }>;
export type IotaObjectChangeCreated = Extract<IotaObjectChange, { type: 'created' }>;
