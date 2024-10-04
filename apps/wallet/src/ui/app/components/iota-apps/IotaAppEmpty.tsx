// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { cva, type VariantProps } from 'class-variance-authority';

const appEmptyStyle = cva(['flex gap-3 p-3.75 h-28'], {
    variants: {
        displayType: {
            full: 'w-full',
            card: 'bg-white flex flex-col p-3.75 box-border w-full rounded-2xl h-32 box-border w-full rounded-2xl border border-solid border-gray-40',
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
            <div className="bg-gray-40 h-10 w-10 rounded-full"></div>
            <div className="flex flex-1 flex-col gap-2.5">
                {styleProps.displayType === 'full' ? (
                    <>
                        <div className="bg-gray-40 h-3.5 w-2/5 rounded"></div>
                        <div className="bg-gray-40 h-3.5 w-full rounded"></div>
                        <div className="bg-gray-40 h-3.5 w-1/4 rounded"></div>
                    </>
                ) : (
                    <div className="flex gap-2">
                        <div className="bg-gray-40 h-3.5 w-1/4 rounded"></div>
                        <div className="bg-gray-40 h-3.5 w-3/5 rounded"></div>
                    </div>
                )}
            </div>
        </div>
    );
}
