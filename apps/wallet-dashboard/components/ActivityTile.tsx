// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import ActivityIcon from './ActivityIcon';
import formatTimestamp from '@/lib/utils/time';
import { Activity } from '@/lib/interfaces';
import { usePopups } from '@/hooks';
import { ActivityDetailsPopup, Button } from '@/components';

interface ActivityTileProps {
    activity: Activity;
}

function ActivityTile({ activity }: ActivityTileProps): JSX.Element {
    const { openPopup, closePopup } = usePopups();

    const handleDetailsClick = () => {
        openPopup(<ActivityDetailsPopup activity={activity} onClose={closePopup} />);
    };

    return (
        <div className="border-gray-45 flex h-full w-full flex-row items-center space-x-4 rounded-md border border-solid p-4">
            <ActivityIcon state={activity.state} action={activity.action} />
            <div className="flex h-full w-full flex-col space-y-2">
                <h2>{activity.action}</h2>
                <span>{formatTimestamp(activity.timestamp)}</span>
            </div>
            <Button onClick={handleDetailsClick}>Details</Button>
        </div>
    );
}

export default ActivityTile;
