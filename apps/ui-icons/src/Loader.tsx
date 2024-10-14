// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgLoader(props: SVGProps<SVGSVGElement>) {
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
                d="M5.533 9.321A7 7 0 1 0 19 12a1 1 0 1 1 2 0 9 9 0 1 1-9-9 1 1 0 1 1 0 2 7 7 0 0 0-6.467 4.321"
                clipRule="evenodd"
            />
        </svg>
    );
}
