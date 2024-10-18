// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Placeholder } from '@iota/apps-ui-kit';
import { cva, type VariantProps } from 'class-variance-authority';

const appEmptyStyle = cva(['flex gap-3 p-lg h-28'], {
    variants: {
        displayType: {
            full: 'w-full',
            card: 'bg-neutral-100 flex flex-col p-lg w-full rounded-2xl h-32 box-border w-full rounded-2xl border border-solid border-shader-primary-dark',
        },
    },
    defaultVariants: {
        displayType: 'full',
    },
});

export interface IotaAppEmptyProps extends VariantProps<typeof appEmptyStyle> {}

export function IotaAppEmpty({ ...styleProps }: IotaAppEmptyProps) {
    return (
        <div className={appEmptyStyle(styleProps)}>
            <Placeholder width="w-10" height="h-10" />
            <div className="flex flex-1 flex-col gap-2.5">
                {styleProps.displayType === 'full' ? (
                    <>
                        {new Array(2).fill(0).map((_, index) => (
                            <Placeholder />
                        ))}
                    </>
                ) : (
                    <div className="flex gap-2">
                        <Placeholder width="w-1/4" height="h-3.5" />
                        <Placeholder width="w-3/5" height="h-3.5" />
                    </div>
                )}
            </div>
        </div>
    );
}
