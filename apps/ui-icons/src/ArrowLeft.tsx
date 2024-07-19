// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgArrowLeft(props: SVGProps<SVGSVGElement>) {
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
                d="M8.165 11.45a1 1 0 0 0 .128 1.257l5 5a1 1 0 0 0 1.414-1.414L10.414 12l4.293-4.293a1 1 0 0 0-1.414-1.414l-5 5a1.006 1.006 0 0 0-.128.157Z"
            />
        </svg>
    );
}
