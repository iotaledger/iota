// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { SVGProps } from 'react';
export default function SvgSubnames(props: SVGProps<SVGSVGElement>) {
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
                d="M7 3a2.998 2.998 0 0 1 .984 5.831A1 1 0 0 1 8 9v3a1 1 0 0 0 1 1h7.585l-2.535-2.536a1 1 0 1 1 1.414-1.414l4.95 4.95-4.95 4.95a1 1 0 0 1-1.414-1.415L16.585 15H9a3 3 0 0 1-3-3V9a1 1 0 0 1 .015-.169A2.998 2.998 0 0 1 7 3m0 2a1 1 0 1 0 0 2 1 1 0 0 0 0-2"
                clipRule="evenodd"
            />
        </svg>
    );
}
