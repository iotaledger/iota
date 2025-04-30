// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgRefresh(props: SVGProps<SVGSVGElement>) {
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
                d="M10.662 5.13A7 7 0 0 1 17.745 8h-1.988a1 1 0 1 0 0 2H20a1 1 0 0 0 1-1V4.757a1 1 0 1 0-2 0v1.586a8.999 8.999 0 1 0 1.301 9.134 1 1 0 1 0-1.845-.773 7 7 0 1 1-7.794-9.575Z"
            />
        </svg>
    );
}
