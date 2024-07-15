// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgLedger(props: SVGProps<SVGSVGElement>) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="1em"
            height="1em"
            fill="none"
            viewBox="0 0 24 24"
            {...props}
        >
            <path
                fill="currentColor"
                d="M3 6a2 2 0 0 1 2-2h3a1 1 0 0 1 0 2H5v3a1 1 0 0 1-2 0V6ZM3 18a2 2 0 0 0 2 2h3a1 1 0 1 0 0-2H5v-3a1 1 0 1 0-2 0v3ZM19 4a2 2 0 0 1 2 2v3a1 1 0 1 1-2 0V6h-3a1 1 0 1 1 0-2h3ZM21 18a2 2 0 0 1-2 2h-3a1 1 0 1 1 0-2h3v-3a1 1 0 1 1 2 0v3ZM12 14V9a1 1 0 1 0-2 0v5a2 2 0 0 0 2 2h2a1 1 0 1 0 0-2h-2Z"
            />
        </svg>
    );
}
