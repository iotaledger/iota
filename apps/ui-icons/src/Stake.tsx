// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgStake(props: SVGProps<SVGSVGElement>) {
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
                d="M9.707 4.05a7 7 0 1 1 9.9 9.9l-6.115 6.076a7 7 0 0 1-9.829-9.899zm1.414 1.414a5 5 0 0 0-1.114 1.694l.005.012-.011.005a5.001 5.001 0 1 0 1.12-1.71Zm-3.947 4.539.01-.004-.005-.01a5 5 0 0 1 .478-.918 6.98 6.98 0 0 0 2.05 4.879A6.98 6.98 0 0 0 14.585 16a5 5 0 0 1-7.411-5.997M4.829 11.9a5 5 0 0 0 6.928 6.928A7 7 0 0 1 4.83 11.9Z"
                clipRule="evenodd"
            />
        </svg>
    );
}
