// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { Card, CardImage, ImageShape, Skeleton } from '@/components';

const meta: Meta<typeof Skeleton> = {
    component: Skeleton,
    tags: ['autodocs'],
} satisfies Meta<typeof Skeleton>;

export default meta;

type Story = StoryObj<typeof meta>;

export const SkeletonCard: Story = {
    render: () => (
        <Card>
            <CardImage shape={ImageShape.SquareRounded}>
                <div className="h-10 w-10 animate-pulse bg-iota-neutral-90 names:bg-names-neutral-12 dark:bg-iota-neutral-12" />
                <Skeleton className="h-10 w-10 rounded-none" />
            </CardImage>
            <div className="flex flex-col gap-y-xs">
                <Skeleton className="h-3.5 w-40" />
                <Skeleton className="h-3 w-32" hasSecondaryColors />
            </div>
            <div className="ml-auto flex flex-col gap-y-xs">
                <Skeleton className="h-3.5 w-20" />
                <Skeleton className="h-3 w-16" hasSecondaryColors />
            </div>
        </Card>
    ),
};
