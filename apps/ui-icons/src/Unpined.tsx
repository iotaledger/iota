// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgUnpined(props: SVGProps<SVGSVGElement>) {
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
                d="M17 3v2h-1v8.175L7.825 5 7 4.175V3zm-5 20-1-1v-6H6v-2l2-2v-1.15L1.4 4.2l1.4-1.4 18.4 18.4-1.45 1.4-6.6-6.6H13v6z"
            />
        </svg>
    );
}
