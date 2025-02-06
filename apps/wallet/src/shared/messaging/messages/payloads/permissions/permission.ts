// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { PermissionType } from './permissionType';

//TODO: add description, name, tags
//TODO add PageLink for instance where the origin and the wallet landing page are different.
export interface Permission {
    name?: string;
    id: string;
    origin: string;
    pagelink?: string | undefined;
    favIcon: string | undefined;
    accounts: string[];
    allowed: boolean | null;
    permissions: PermissionType[];
    createdDate: string;
    responseDate: string | null;
    requestMsgID: string;
}
