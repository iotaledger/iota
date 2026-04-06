// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { SVGProps } from 'react';
export default function SvgWallet(props: SVGProps<SVGSVGElement>) {
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
                fillRule="evenodd"
                d="M3.175 18.825Q4.35 20 6 20h12q1.65 0 2.825-1.175T22 16V8q0-1.65-1.175-2.825T18 4H6Q4.35 4 3.175 5.175T2 8v8q0 1.65 1.175 2.825M18 8H6q-.55 0-1.05.125a3.4 3.4 0 0 0-.95.4V8q0-.824.588-1.412A1.93 1.93 0 0 1 6 6h12q.824 0 1.413.588Q20 7.175 20 8v.525a3.4 3.4 0 0 0-.95-.4A4.3 4.3 0 0 0 18 8m2 5a2 2 0 1 1-4 0 2 2 0 0 1 4 0"
                clipRule="evenodd"
            />
        </svg>
    );
}
