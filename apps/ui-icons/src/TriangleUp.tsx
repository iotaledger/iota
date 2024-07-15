// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgTriangleUp(props: SVGProps<SVGSVGElement>) {
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
                d="M7.65 15c-.578 0-.867-.681-.459-1.08l4.85-4.735a.66.66 0 0 1 .917 0l4.85 4.735c.41.399.12 1.08-.457 1.08H7.649Z"
            />
        </svg>
    );
}
