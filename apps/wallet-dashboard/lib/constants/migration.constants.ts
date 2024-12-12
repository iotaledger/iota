// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { StardustObjectTypeFilter } from '../enums';

export const STARDUST_MIGRATABLE_OBJECTS_FILTER_LIST: StardustObjectTypeFilter[] =
    Object.values(StardustObjectTypeFilter);

export const STARDUST_UNMIGRATABLE_OBJECTS_FILTER_LIST: StardustObjectTypeFilter[] = Object.values(
    StardustObjectTypeFilter,
).filter((element) => element !== StardustObjectTypeFilter.WithExpiration);

export const MIGRATION_OBJECT_WITHOUT_EXPIRATION_KEY = 'no-expiration';
