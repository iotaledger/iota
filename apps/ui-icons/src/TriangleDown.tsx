// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgTriangleDown(props: SVGProps<SVGSVGElement>) {
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
                d="M17.35 9c.578 0 .867.681.459 1.08l-4.85 4.735a.66.66 0 0 1-.917 0l-4.85-4.735C6.781 9.68 7.071 9 7.648 9h9.702Z"
            />
        </svg>
    );
}
