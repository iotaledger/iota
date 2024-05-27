// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import ActivityIcon from './ActivityIcon';
import formatTimestamp from '@/lib/utils/time';
import { Activity } from '@/lib/interfaces';

interface ActivityTileProps {
  activity: Activity
}

function ActivityTile({ activity: { action, state, timestamp } }: ActivityTileProps): JSX.Element {
  return (
    <div className='flex flex-row items-center space-x-4 rounded-md border border-solid border-gray-45 p-4 w-full h-full'>
      <ActivityIcon
          state={state}
          action={action}
      />
      <div className='flex flex-col space-y-2 h-full'>
        <h2>{action}</h2>
        <span>{formatTimestamp(timestamp)}</span>
      </div>
      <hr />
    </div>
  );
};

export default ActivityTile;
