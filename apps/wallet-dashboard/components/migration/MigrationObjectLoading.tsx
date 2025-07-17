// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Card, CardImage, ImageShape, Skeleton } from '@iota/apps-ui-kit';

export function MigrationObjectLoading() {
    return (
        <div className="flex h-full max-h-full w-full flex-col overflow-hidden">
            {new Array(10).fill(0).map((_, index) => (
                <Card key={index}>
                    <CardImage shape={ImageShape.SquareRounded}>
                        <div className="h-10 w-10 animate-pulse bg-iota-neutral-90 dark:bg-iota-neutral-12" />
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
            ))}
        </div>
    );
}
