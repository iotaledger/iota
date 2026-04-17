// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LabelText, LabelTextSize, Panel, Title } from '@iota/apps-ui-kit';
import { formatDate, useTimeAgo } from '@iota/core';
import { LinkWithQuery, ProgressBar } from '~/components/ui';
import { useDateFormat } from '~/contexts/dateFormatContext';
import { useGetNetworkMetrics } from '~/hooks';
import { ampli } from '~/lib/utils';
import { useEpochProgress } from '~/pages/epochs/utils';

function useEpochDateSubtitle(start?: number, end?: number, progress?: number, label?: string) {
    const { format } = useDateFormat('epoch');

    const timestamp = !progress && end ? end : start;
    const prefix = !progress && end ? 'End' : 'Started';

    const relativeText = useTimeAgo({ timeFrom: timestamp ?? null, shortedTimeLabel: false });
    const timeZone = format === 'utc' ? 'UTC' : undefined;
    const absoluteText = timestamp
        ? formatDate(timestamp, ['day', 'month', 'year', 'hour', 'minute', 'second'], timeZone)
        : null;

    const dateText = format === 'default' ? relativeText : absoluteText;

    return !progress && label ? label : dateText ? `${prefix} ${dateText}` : '--';
}

export function CurrentEpoch(): JSX.Element {
    const { epoch, progress, label, end, start } = useEpochProgress();
    const { data: networkData } = useGetNetworkMetrics();

    const subtitle = useEpochDateSubtitle(start, end, progress, label);

    return (
        <LinkWithQuery
            className="flex w-full"
            to={`/epoch/${epoch}`}
            onClick={() => ampli.clickedCurrentEpochCard({ epoch: Number(epoch) })}
        >
            <Panel>
                <Title title={`Epoch ${epoch ?? '--'}`} subtitle={subtitle} />
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
