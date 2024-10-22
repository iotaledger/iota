// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgUnstake(props: SVGProps<SVGSVGElement>) {
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
                d="m19.607 13.95-1.464 1.455-1.747-1.716a5 5 0 1 0-6.39-6.53l.006.011-.011.005-.063.168-1.773-1.742 1.542-1.55a7 7 0 1 1 9.9 9.899"
            />
            <path
                fill="currentColor"
                fillRule="evenodd"
                d="M3.663 10.127 4.675 9.11l2.185 2.15a5 5 0 0 0 5.624 5.526l2.146 2.109-1.14 1.132a7 7 0 0 1-9.828-9.899ZM4.83 11.9a5 5 0 0 0 6.928 6.928A7 7 0 0 1 4.83 11.9"
                clipRule="evenodd"
            />
            <path fill="currentColor" d="M19.086 21.414 2 4.414 3.414 3 20.5 20z" />
        </svg>
    );
}
