// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgVesting(props: SVGProps<SVGSVGElement>) {
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
                stroke="currentColor"
                strokeLinecap="round"
                strokeWidth={2}
                d="M20 13V7a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h4"
            />
            <path fill="currentColor" d="M4 5h16v3H4z" />
            <rect width={4} height={2} x={7} y={10} fill="currentColor" rx={1} />
            <rect
                width={4}
                height={2}
                x={9}
                y={2}
                fill="currentColor"
                rx={1}
                transform="rotate(90 9 2)"
            />
            <rect
                width={4}
                height={2}
                x={17}
                y={2}
                fill="currentColor"
                rx={1}
                transform="rotate(90 17 2)"
            />
            <circle cx={16} cy={19} r={4} fill="currentColor" />
            <path
                stroke="currentColor"
                strokeLinecap="round"
                strokeWidth={2}
                d="M17 19a3 3 0 1 0 .102-.776"
            />
        </svg>
    );
}
