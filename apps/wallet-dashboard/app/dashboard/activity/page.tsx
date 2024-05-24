// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import ActivityTile from "@/components/ActivityTile"

function StakingDashboardPage(): JSX.Element {

    const MOCK_ACTIVITIES = [
      {
        action: "Send",
        success: true,
        timestamp: 1716538921485
      },
      {
        action: "Transaction",
        success: true,
        timestamp: 1715868828552
      },
      {
        action: "Send",
        success: true,
        timestamp: 1712186639729
      },
      {
        action: "Rewards",
        success: true,
        timestamp: 1715868828552
      },
      {
        action: "Receive",
        success: true,
        timestamp: 1712186639729
      },
      {
        action: "Transaction",
        success: true,
        timestamp: 1715868828552
      },
      {
        action: "Send",
        success: false,
        timestamp: 1712186639729
      },
    ]

    return (
        <div className="flex flex-col items-center justify-center pt-12 space-y-4">
            <h1>Your Activity</h1>
            {MOCK_ACTIVITIES.map((activity) => 
                <ActivityTile {...activity} />
              )}
        </div>
    )
}

export default StakingDashboardPage