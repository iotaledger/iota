// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgDataStack(props: SVGProps<SVGSVGElement>) {
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
                d="M5 14v2c0 1.657 3.134 3 7 3s7-1.343 7-3v-2c0 1.657-3.134 3-7 3s-7-1.343-7-3Z"
            />
            <path
                fill="currentColor"
                d="M5 10v2c0 1.657 3.134 3 7 3s7-1.343 7-3v-2c0 1.657-3.134 3-7 3s-7-1.343-7-3Z"
            />
            <path
                fill="currentColor"
                d="M19 8c0 1.657-3.134 3-7 3S5 9.657 5 8s3.134-3 7-3 7 1.343 7 3Z"
            />
        </svg>
    );
}
