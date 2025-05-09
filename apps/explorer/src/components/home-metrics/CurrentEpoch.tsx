// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LabelText, LabelTextSize, Panel, Title } from '@iota/apps-ui-kit';
import { formatDate } from '@iota/core';
import { format, isToday, isYesterday } from 'date-fns';
import { useMemo } from 'react';

import { LinkWithQuery, ProgressBar } from '~/components/ui';
import { useGetNetworkMetrics } from '~/hooks';
import { ampli } from '~/lib/utils';
import { useEpochProgress } from '~/pages/epochs/utils';

export function CurrentEpoch(): JSX.Element {
    const { epoch, progress, label, end, start } = useEpochProgress();
    const { data: networkData } = useGetNetworkMetrics();

    const formattedDateString = useMemo(() => {
        if (!start) {
            return null;
        }

        let formattedDate = '';
        const epochStartDate = new Date(start);
        if (isToday(epochStartDate)) {
            formattedDate = 'Today';
        } else if (isYesterday(epochStartDate)) {
            formattedDate = 'Yesterday';
        } else {
            formattedDate = format(epochStartDate, 'PPP');
        }
        const formattedTime = format(epochStartDate, 'p');
        return `${formattedTime}, ${formattedDate}`;
    }, [start]);

    const epochSubtitle =
        !progress && end
            ? `End ${formatDate(end)}`
            : formattedDateString
              ? `Started ${formattedDateString}`
              : '--';

    return (
        <LinkWithQuery
            className="flex w-full"
            to={`/epoch/${epoch}`}
            onClick={() => ampli.clickedCurrentEpochCard({ epoch: Number(epoch) })}
        >
            <Panel>
                <Title title={`Epoch ${epoch ?? '--'}`} subtitle={epochSubtitle} />
                <div className="flex flex-col gap-md p-md--rs">
                    <div className="flex flex-row gap-md">
                        <div className="flex flex-1">
                            <LabelText
                                size={LabelTextSize.Large}
                                label="Time Left"
                                text={label || '--'}
                            />
                        </div>
                        <div className="flex flex-1">
                            <LabelText
                                size={LabelTextSize.Large}
                                label="Checkpoint"
                                text={
                                    networkData?.currentCheckpoint
                                        ? BigInt(
                                              networkData.currentCheckpoint || 0,
                                          ).toLocaleString()
                                        : '--'
                                }
                            />
                        </div>
                    </div>
                    <ProgressBar progress={progress || 0} />
                </div>
            </Panel>
        </LinkWithQuery>
    );
}
