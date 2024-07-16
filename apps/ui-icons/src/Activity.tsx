// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgActivity(props: SVGProps<SVGSVGElement>) {
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
                d="M16.811 10.584a1 1 0 1 0-1.623-1.168l-1.852 2.573-1.694-1.059a2 2 0 0 0-2.547.358l-1.838 2.043a1 1 0 0 0 1.486 1.338l1.839-2.043 1.694 1.059a2 2 0 0 0 2.683-.527l1.852-2.574Z"
            />
            <path
                fill="currentColor"
                fillRule="evenodd"
                d="M3 9a6 6 0 0 1 6-6h6a6 6 0 0 1 6 6v6a6 6 0 0 1-6 6H9a6 6 0 0 1-6-6V9Zm16 0v6a4 4 0 0 1-4 4H9a4 4 0 0 1-4-4V9a4 4 0 0 1 4-4h6a4 4 0 0 1 4 4Z"
                clipRule="evenodd"
            />
        </svg>
    );
}
