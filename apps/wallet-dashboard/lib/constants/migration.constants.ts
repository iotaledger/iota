// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { StardustOutputDetailsFilter } from '../enums';

export const STARDUST_MIGRATABLE_OBJECTS_FILTER_LIST: StardustOutputDetailsFilter[] = Object.values(
    StardustOutputDetailsFilter,
);

export const STARDUST_TIMELOCKED_OBJECTS_FILTER_LIST: StardustOutputDetailsFilter[] =
    Object.values(StardustOutputDetailsFilter).filter(
        (element) => element !== StardustOutputDetailsFilter.WithExpiration,
    );

export const MIGRATION_OBJECT_WITHOUT_UC_KEY = 'no-unlock-condition-timestamp';
