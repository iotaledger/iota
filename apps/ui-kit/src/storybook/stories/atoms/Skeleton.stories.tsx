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
                <Skeleton widthClass="w-10" heightClass="h-10" isRounded={false} />
            </CardImage>
            <div className="flex flex-col gap-y-xs">
                <Skeleton widthClass="w-40" heightClass="h-3.5" />
                <Skeleton widthClass="w-32" heightClass="h-3" hasSecondaryColors />
            </div>
            <div className="ml-auto flex flex-col gap-y-xs">
                <Skeleton widthClass="w-20" heightClass="h-3.5" />
                <Skeleton widthClass="w-16" heightClass="h-3" hasSecondaryColors />
            </div>
        </Card>
    ),
};
