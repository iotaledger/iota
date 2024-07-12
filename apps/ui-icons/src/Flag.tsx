// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgFlag(props: SVGProps<SVGSVGElement>) {
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
                d="M7 4a1 1 0 0 0-1 1v15a1 1 0 1 0 2 0v-7h9a1 1 0 0 0 1-1V7a1 1 0 0 0-1-1H8V5a1 1 0 0 0-1-1Z"
            />
        </svg>
    );
}
