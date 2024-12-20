// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Card, CardImage, ImageShape, Skeleton } from '@iota/apps-ui-kit';

export function MigrationObjectLoading() {
    return (
        <div className="flex h-full max-h-full w-full flex-col overflow-hidden">
            {new Array(10).fill(0).map((_, index) => (
                <Card key={index}>
                    <CardImage shape={ImageShape.SquareRounded}>
                        <div className="h-10 w-10 animate-pulse bg-neutral-90 dark:bg-neutral-12" />
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
            ))}
        </div>
    );
}
