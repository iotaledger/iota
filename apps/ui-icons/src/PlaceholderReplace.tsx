// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgPlaceholderReplace(props: SVGProps<SVGSVGElement>) {
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
                d="M11 9a2 2 0 1 1-4 0 2 2 0 0 1 4 0ZM15 11a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z"
            />
            <path
                fill="currentColor"
                fillRule="evenodd"
                d="M2 8a6 6 0 0 1 6-6h8a6 6 0 0 1 6 6v8a6 6 0 0 1-6 6H8a6 6 0 0 1-6-6V8Zm2 0a4 4 0 0 1 4-4h8a4 4 0 0 1 4 4v3.871a19.125 19.125 0 0 1-16 0V8Zm0 6.051V16a4 4 0 0 0 4 4h8a4 4 0 0 0 4-4v-1.949a21.126 21.126 0 0 1-16 0Z"
                clipRule="evenodd"
            />
        </svg>
    );
}
