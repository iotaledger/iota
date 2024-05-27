// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import formatDate from '../lib/utils/formatDate';
import ActivityIcon from './ActivityIcon';

interface ActivityTileProps {
    action:  "Send" | "Receive" | "Transaction" | "Staked" | "Unstaked" | "Rewards" | "Swapped" | "Failed" | "PersonalMessage"
    success: boolean
    timestamp: number
    error?: string
}

function ActivityTile({ action, timestamp, success, error }: ActivityTileProps): JSX.Element {
  return (
    <div className='flex flex-row items-center space-x-4 rounded-md border border-solid border-gray-45 p-4 w-full h-full'>
      <ActivityIcon
          transactionFailed={!success || !!error}
          action={action}
      />
      <div className='flex flex-col space-y-2 h-full'>
        <h2>{action}</h2>
        <span>{formatDate(timestamp)}</span>
      </div>
      <hr />
    </div>
  );
};

export default ActivityTile;
