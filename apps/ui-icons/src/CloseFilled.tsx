// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { SVGProps } from 'react';
export default function SvgCloseFilled(props: SVGProps<SVGSVGElement>) {
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
                d="M12 2c5.523 0 10 4.477 10 10s-4.477 10-10 10S2 17.523 2 12 6.477 2 12 2m4.242 5.757a1 1 0 0 0-1.414 0L12 10.586l-2.829-2.83a1 1 0 0 0-1.414 1.416L10.585 12l-2.828 2.828a1 1 0 0 0 1.414 1.414l2.83-2.828 2.827 2.828a1 1 0 1 0 1.414-1.414L13.414 12l2.828-2.828a1 1 0 0 0 0-1.415"
                clipRule="evenodd"
            />
        </svg>
    );
}
