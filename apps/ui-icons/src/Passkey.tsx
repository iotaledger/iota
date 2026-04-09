// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { SVGProps } from 'react';
export default function SvgPasskey(props: SVGProps<SVGSVGElement>) {
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
                d="M18.5 9.5q1.45 0 2.475 1.025A3.37 3.37 0 0 1 22 13a3.3 3.3 0 0 1-.638 2q-.637.875-1.612 1.25v3.25h1.225v2H17.5v-5.15a3.45 3.45 0 0 1-1.8-1.238Q15 14.2 15 13q0-1.45 1.025-2.475A3.37 3.37 0 0 1 18.5 9.5m0 2a1.5 1.5 0 1 0 0 3 1.5 1.5 0 0 0 0-3"
                clipRule="evenodd"
            />
            <path
                fill="currentColor"
                d="M11 12.5a13.4 13.4 0 0 1 2 .15q-.1 1.45.525 2.738A5.4 5.4 0 0 0 15.35 17.5v2H3v-2.8q0-.85.438-1.562A2.9 2.9 0 0 1 4.6 14.05a15 15 0 0 1 3.15-1.162A13.8 13.8 0 0 1 11 12.5m0-9q1.65 0 2.825 1.175T15 7.5c-.001 1.65-.392 2.042-1.175 2.825Q12.65 11.501 11 11.5c-1.65-.001-2.042-.392-2.825-1.175Q6.999 9.15 7 7.5c.001-1.65.391-2.042 1.175-2.825Q9.35 3.499 11 3.5"
            />
        </svg>
    );
}
