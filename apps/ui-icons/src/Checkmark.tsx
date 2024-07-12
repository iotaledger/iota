// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgCheckmark(props: SVGProps<SVGSVGElement>) {
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
                fillRule="evenodd"
                d="m10.26 13.845 5.95-5.95a.994.994 0 0 1 1.405-.008.994.994 0 0 1-.008 1.406L10.95 15.95a.994.994 0 0 1-1.406.008l-2.846-2.845a.994.994 0 0 1 .01-1.406.994.994 0 0 1 1.405-.009l2.147 2.147Z"
                clipRule="evenodd"
            />
        </svg>
    );
}
