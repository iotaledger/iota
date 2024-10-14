// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgTriangleLeft(props: SVGProps<SVGSVGElement>) {
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
                d="M14 7.15c0-.578-.681-.867-1.08-.459l-4.735 4.85a.66.66 0 0 0 0 .917l4.735 4.85c.399.41 1.08.12 1.08-.457z"
            />
        </svg>
    );
}
