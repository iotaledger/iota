// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgArrowBottomLeft(props: SVGProps<SVGSVGElement>) {
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
                d="M6.296 17.488a.996.996 0 0 0 .704.29h8.071a1 1 0 1 0 0-2H9.414l8.071-8.07a1 1 0 0 0-1.414-1.415L8 14.364V8.707a1 1 0 1 0-2 0v8.07a1 1 0 0 0 .291.706l.002.002.003.003Z"
            />
        </svg>
    );
}
