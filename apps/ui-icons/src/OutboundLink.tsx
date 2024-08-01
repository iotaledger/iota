// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgOutboundLink(props: SVGProps<SVGSVGElement>) {
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
                d="M5 6a1 1 0 0 1 1-1h2a1 1 0 0 0 0-2H6a3 3 0 0 0-3 3v12a3 3 0 0 0 3 3h12a3 3 0 0 0 3-3v-2a1 1 0 1 0-2 0v2a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V6Z"
            />
            <path
                fill="currentColor"
                d="M10.515 12.071 17.585 5H13a1 1 0 1 1 0-2h7a1 1 0 0 1 1 1v7a1 1 0 0 1-2 0V6.414l-7.071 7.071a1 1 0 0 1-1.414-1.414Z"
            />
        </svg>
    );
}
