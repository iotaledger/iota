// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgImageIcon(props: SVGProps<SVGSVGElement>) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="1em"
            height="1em"
            fill="none"
            viewBox="0 0 24 24"
            {...props}
        >
            <path fill="currentColor" d="M15 11a2 2 0 1 0 0-4 2 2 0 0 0 0 4" />
            <path
                fill="currentColor"
                fillRule="evenodd"
                d="M7 3a4 4 0 0 0-4 4v10a4 4 0 0 0 4 4h10a4 4 0 0 0 4-4V7a4 4 0 0 0-4-4zm10 2H7a2 2 0 0 0-2 2v5.634l2.994-1.437a2 2 0 0 1 2.132.255l4.396 3.597L19 11.629V7a2 2 0 0 0-2-2M5 17v-2.148L8.86 13l4.395 3.596a2 2 0 0 0 2.48.042L19 14.145V17a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2"
                clipRule="evenodd"
            />
        </svg>
    );
}
