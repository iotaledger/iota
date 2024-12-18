// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgListViewSmall(props: SVGProps<SVGSVGElement>) {
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
                d="M9 5a1 1 0 0 0 0 2h10a1 1 0 1 0 0-2zm0 6a1 1 0 1 0 0 2h10a1 1 0 1 0 0-2zm-1 7a1 1 0 0 1 1-1h10a1 1 0 1 1 0 2H9a1 1 0 0 1-1-1m-3-7a1 1 0 1 0 0 2 1 1 0 0 0 0-2M4 6a1 1 0 1 1 2 0 1 1 0 0 1-2 0m1 11a1 1 0 1 0 0 2 1 1 0 0 0 0-2"
            />
        </svg>
    );
}
