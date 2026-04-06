// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { SVGProps } from 'react';
export default function SvgSmX(props: SVGProps<SVGSVGElement>) {
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
                d="M5.28 1.92a3.36 3.36 0 0 0-3.36 3.36v13.44a3.36 3.36 0 0 0 3.36 3.36h13.44a3.36 3.36 0 0 0 3.36-3.36V5.28a3.36 3.36 0 0 0-3.36-3.36zm1 4.32h3.81l2.706 3.845L16.08 6.24h1.2l-3.941 4.614 4.86 6.906h-3.81l-3.14-4.46-3.81 4.46h-1.2l4.469-5.23zm1.838.96 6.771 9.6h1.471L9.59 7.2z"
            />
        </svg>
    );
}
