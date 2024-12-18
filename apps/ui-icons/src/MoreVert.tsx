// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgMoreVert(props: SVGProps<SVGSVGElement>) {
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
                d="M10 5a2 2 0 1 0 4 0 2 2 0 0 0-4 0m2 9a2 2 0 1 1 0-4 2 2 0 0 1 0 4m0 7a2 2 0 1 1 0-4 2 2 0 0 1 0 4"
            />
        </svg>
    );
}
