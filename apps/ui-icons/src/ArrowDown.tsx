// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgArrowDown(props: SVGProps<SVGSVGElement>) {
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
                d="M11.45 15.335a1 1 0 0 0 1.257-.128l5-5a1 1 0 0 0-1.414-1.414L12 13.086 7.707 8.793a1 1 0 0 0-1.414 1.414l5 5c.048.049.101.092.157.128Z"
            />
        </svg>
    );
}
