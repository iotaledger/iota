// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LabelText, LoadingIndicator, Panel, Title, TitleSize } from '@iota/apps-ui-kit';
import type { ComponentProps } from 'react';
import { AreaGraph } from './AreaGraph';
import { ErrorBoundary } from './error-boundary';
import { ParentSize } from '@visx/responsive';

type StatisticsPanelProps<T> = {
    title: string;
    data?: ComponentProps<typeof AreaGraph<T>>['data'];
    stats: ComponentProps<typeof LabelText>[];
    isPending?: boolean;
} & Omit<ComponentProps<typeof AreaGraph<T>>, 'data' | 'width' | 'height'>;

export function StatisticsPanel<T>({
    title,
    data,
    stats,
    isPending,
    getX,
    getY,
    formatX,
    formatY,
    tooltipContent,
}: StatisticsPanelProps<T>): React.JSX.Element {
    return (
        <Panel>
            <Title title={title} size={TitleSize.Medium} />
            <div className="flex h-full flex-col gap-md p-md--rs">
                <div className="grid grid-cols-2 gap-md">
                    {stats.map((stat, index) => (
                        <LabelText key={index} {...stat} />
                    ))}
                </div>

                <div className="mt-auto flex max-h-[270px] min-h-[180px] flex-1 flex-col items-center justify-center rounded-xl transition-colors">
                    {isPending ? (
                        <LoadingIndicator text="Loading data" />
                    ) : data?.length ? (
                        <div className="relative flex-1 self-stretch">
                            <ErrorBoundary>
                                <ParentSize className="absolute">
                                    {({ height, width }) => (
                                        <AreaGraph
                                            data={data}
                                            height={height}
                                            width={width}
                                            getX={getX}
                                            getY={getY}
                                            formatX={formatX}
                                            formatY={formatY}
                                            tooltipContent={tooltipContent}
                                        />
                                    )}
                                </ParentSize>
                            </ErrorBoundary>
                        </div>
                    ) : (
                        <div className="flex items-center justify-center">
                            <span className="flex flex-row items-center gap-x-xs text-neutral-40 dark:text-neutral-60">
                                No historical data available
                            </span>
                        </div>
                    )}
                </div>
            </div>
        </Panel>
    );
}

interface GraphTooltipProps {
    title: string;
    overline: string;
    subtitle: string;
}
export function GraphTooltip({ title, overline, subtitle }: GraphTooltipProps) {
    return (
        <div className="flex flex-col gap-xxxs">
            <span className="text-body-sm text-neutral-40">{overline}</span>
            <span className="text-label-lg text-neutral-12">{title}</span>
            <span className="text-body-sm text-neutral-40">{subtitle}</span>
        </div>
    );
}
