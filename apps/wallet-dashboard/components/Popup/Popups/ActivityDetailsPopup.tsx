// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Activity } from '@/lib/interfaces';

interface ActivityDetailsPopupProps {
    activity: Activity;
    onClose: () => void;
}

function ActivityDetailsPopup({ activity, onClose }: ActivityDetailsPopupProps): JSX.Element {
    return (
        <div className="flex w-full min-w-[300px] flex-col gap-2">
            <h2>Transaction Details</h2>
            <p>Action: {activity.action}</p>
            <p>State: {activity.state}</p>
            <p>Timestamp: {new Date(activity.timestamp).toLocaleString()}</p>
        </div>
    );
}

export default ActivityDetailsPopup;
