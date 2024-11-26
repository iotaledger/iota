// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgListViewMedium(props: SVGProps<SVGSVGElement>) {
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
                d="M3 7a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v2a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2zm4 0H5v2h2zm-4 8a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v2a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2zm4 0H5v2h2z"
                clipRule="evenodd"
            />
            <path
                fill="currentColor"
                d="M12 11a1 1 0 1 0 0 2h7a1 1 0 1 0 0-2zm-1-5a1 1 0 0 1 1-1h7a1 1 0 1 1 0 2h-7a1 1 0 0 1-1-1m1 11a1 1 0 1 0 0 2h7a1 1 0 1 0 0-2z"
            />
        </svg>
    );
}
