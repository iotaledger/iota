// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgHandler(props: SVGProps<SVGSVGElement>) {
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
                d="M19.354 11.354a.5.5 0 0 0-.708-.708l-8 8a.5.5 0 0 0 .708.708zm0 4a.5.5 0 0 0-.708-.708l-4 4a.5.5 0 0 0 .708.708z"
            />
        </svg>
    );
}
