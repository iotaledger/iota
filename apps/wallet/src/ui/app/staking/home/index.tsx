// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Route, Routes } from 'react-router-dom';
import { useUnlockedGuard } from '_hooks';
import { DelegationDetail } from '../delegation-detail';
import { StakePage } from '../stake';
import { Validators } from '../validators';

export function StakingPage() {
    if (useUnlockedGuard()) {
        return null;
    }
    return (
        <Routes>
            <Route path="/*" element={<Validators />} />
            <Route path="/delegation-detail" element={<DelegationDetail />} />
            <Route path="/new" element={<StakePage />} />
        </Routes>
    );
}
