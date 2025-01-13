// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Route, Routes } from 'react-router-dom';
import { TokenDetailsPage } from './TokenDetailsPage';
import { TokenDetails } from './TokensDetails';

export function TokensPage() {
    return (
        <Routes>
            <Route path="/" element={<TokenDetails />} />
            <Route path="/details" element={<TokenDetailsPage />} />
        </Routes>
    );
}
