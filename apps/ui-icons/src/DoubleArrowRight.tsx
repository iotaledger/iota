// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgDoubleArrowRight(props: SVGProps<SVGSVGElement>) {
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
                d="M11.835 11.45a1 1 0 0 1-.128 1.257l-5 5a1 1 0 0 1-1.414-1.414L9.586 12 5.293 7.707a1 1 0 0 1 1.414-1.414l5 5c.049.048.092.101.128.157Z"
            />
            <path
                fill="currentColor"
                d="M17.835 11.45a1 1 0 0 1-.128 1.257l-5 5a1 1 0 0 1-1.414-1.414L15.586 12l-4.293-4.293a1 1 0 0 1 1.414-1.414l5 5c.049.048.092.101.128.157Z"
            />
        </svg>
    );
}
