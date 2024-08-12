// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Route, Routes } from 'react-router-dom';

import { useUnlockedGuard } from '_app/hooks/useUnlockedGuard';
import { DelegationDetail } from '_app/staking/delegation-detail';
import StakePage from '_app/staking/stake';
import { Validators } from '_app/staking/validators';

export function Staking() {
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
