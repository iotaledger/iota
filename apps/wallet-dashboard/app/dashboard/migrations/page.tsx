// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client'

import { useDeriveAddress } from "@mysten/dapp-kit";

function StakingDashboardPage(): JSX.Element {
    const derive = useDeriveAddress();

	console.log(derive)
    return (
        <div className="flex items-center justify-center pt-12">
            <h1>MIGRATIONS</h1>
        </div>
    )
}

export default StakingDashboardPage