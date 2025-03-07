// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgExpand(props: SVGProps<SVGSVGElement>) {
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
                d="M4 2a2 2 0 0 0-2 2v4a1 1 0 1 0 2 0V5.414l3.293 3.293a1 1 0 0 0 1.414-1.414L5.414 4H8a1 1 0 0 0 0-2zm18 2a2 2 0 0 0-2-2h-4a1 1 0 1 0 0 2h2.586l-3.293 3.293a1 1 0 0 0 1.414 1.414L20 5.414V8a1 1 0 1 0 2 0zM2 20a2 2 0 0 0 2 2h4a1 1 0 1 0 0-2H5.414l3.293-3.293a1 1 0 1 0-1.414-1.414L4 18.586V16a1 1 0 1 0-2 0zm20 0a2 2 0 0 1-2 2h-4a1 1 0 1 1 0-2h2.586l-3.293-3.293a1 1 0 0 1 1.414-1.414L20 18.586V16a1 1 0 1 1 2 0z"
            />
        </svg>
    );
}
