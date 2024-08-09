// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Outlet } from 'react-router-dom';

import { Toaster } from '_app/shared/toaster';

export function AccountsPage() {
    return (
        <>
            <Outlet />
            <Toaster />
        </>
    );
}
