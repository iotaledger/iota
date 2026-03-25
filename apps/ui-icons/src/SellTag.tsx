// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { SVGProps } from 'react';
export default function SvgSellTag(props: SVGProps<SVGSVGElement>) {
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
                d="M8.5 7q.624 0 1.063.438Q10 7.875 10 8.5q0 .624-.438 1.063A1.45 1.45 0 0 1 8.5 10q-.624 0-1.063-.438A1.45 1.45 0 0 1 7 8.5q0-.624.438-1.063A1.45 1.45 0 0 1 8.5 7"
            />
            <path
                fill="currentColor"
                fillRule="evenodd"
                d="M11.892 3.01a2 2 0 0 1 1.167.56l8.37 8.384.133.147q.184.226.283.494.13.356.13.713l-.007.177q-.026.264-.123.523-.13.345-.416.63l-6.801 6.792-.148.134a1.9 1.9 0 0 1-1.208.436l-.178-.009a1.97 1.97 0 0 1-1.03-.427l-.147-.134-8.395-8.384a1.88 1.88 0 0 1-.548-1.33V4.9q0-.686.43-1.199l.13-.142Q4.093 3.001 4.878 3h6.825zm-6.916 8.66 8.296 8.286 6.67-6.661L11.66 5H4.976z"
                clipRule="evenodd"
            />
        </svg>
    );
}
