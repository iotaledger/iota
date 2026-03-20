// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { SVGProps } from 'react';
export default function SvgLoader2(props: SVGProps<SVGSVGElement>) {
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
                d="M12 18a2 2 0 1 1 0 4 2 2 0 0 1 0-4m-5.656-2.343a2 2 0 1 1-.001 4.001 2 2 0 0 1 0-4Zm11.313 0a2 2 0 1 1 0 4 2 2 0 0 1 0-4M4 10a2 2 0 1 1 0 4 2 2 0 0 1 0-4m16 0a2 2 0 1 1 0 4 2 2 0 0 1 0-4M6.344 4.344a2 2 0 1 1-.001 4 2 2 0 0 1 0-4Zm11.313-.001a2 2 0 1 1 0 4 2 2 0 0 1 0-4M12 2a2 2 0 1 1 0 4 2 2 0 0 1 0-4"
            />
        </svg>
    );
}
