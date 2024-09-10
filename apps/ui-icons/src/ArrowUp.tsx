// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgArrowUp(props: SVGProps<SVGSVGElement>) {
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
                d="m6.293 13.793 5-5a1 1 0 0 1 1.414 0l5 5a1 1 0 0 1-1.414 1.414L12 10.914l-4.293 4.293a1 1 0 0 1-1.414-1.414Z"
            />
        </svg>
    );
}
