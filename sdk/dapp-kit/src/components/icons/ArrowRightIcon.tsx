// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ComponentProps } from 'react';

export function ArrowRightIcon(props: ComponentProps<'svg'>) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            {...props}
        >
            <path
                fill="currentColor"
                d="m10.707 6.293 5 5a1 1 0 0 1 0 1.414l-5 5a1 1 0 0 1-1.414-1.414L13.586 12 9.293 7.707a1 1 0 0 1 1.414-1.414Z"
            />
        </svg>
    );
}
