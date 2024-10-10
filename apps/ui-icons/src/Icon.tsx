// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgIcon(props: SVGProps<SVGSVGElement>) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="1em"
            height="1em"
            fill="none"
            viewBox="0 0 18 18"
            {...props}
        >
            <path
                fill="currentColor"
                fillRule="evenodd"
                d="M9 18A9 9 0 1 0 9 0a9 9 0 0 0 0 18m0-3.2a1.2 1.2 0 1 0 0-2.4 1.2 1.2 0 0 0 0 2.4M7.8 5.2a1.2 1.2 0 0 1 2.4 0v3.6a1.2 1.2 0 0 1-2.4 0z"
                clipRule="evenodd"
            />
        </svg>
    );
}
