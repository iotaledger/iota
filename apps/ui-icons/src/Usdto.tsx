// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { SVGProps } from 'react';
export default function SvgUsdto(props: SVGProps<SVGSVGElement>) {
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
                d="M14.077 23H9.923v-4.308h4.154zm-4.154-4.308H5.77v-8.077h4.154zm8.307 0h-4.153v-8.077h4.154v8.077ZM21 6.308h-6.923v4.307H9.923V6.308H3V2h18z"
            />
        </svg>
    );
}
