// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgAssets(props: SVGProps<SVGSVGElement>) {
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
                d="M7 3a5 5 0 0 0-5 5v6a5 5 0 0 0 5 5c1.038 0 1.63.263 2.605.524l5.795 1.553a5 5 0 0 0 6.124-3.536l1.553-5.795a5 5 0 0 0-3.536-6.124l-2.597-.696A5 5 0 0 0 13 3zm10.989 4.659a5 5 0 0 0-.043-.394l1.078.289a3 3 0 0 1 2.121 3.674l-1.553 5.796a3 3 0 0 1-3.674 2.121l-1.408-.377a5 5 0 0 0 2.916-2.44A5 5 0 0 0 18 14V8q0-.172-.011-.341m-2.54 8.074c.347-.49.551-1.087.551-1.733V8a3 3 0 0 0-3-3H7a3 3 0 0 0-3 3v6a3 3 0 0 0 2.984 3H13c1.011 0 1.906-.5 2.45-1.267Z"
                clipRule="evenodd"
            />
        </svg>
    );
}
