// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgCircleEmitter(props: SVGProps<SVGSVGElement>) {
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
                d="M4.609 8.938a8 8 0 0 0 1.734 8.719L4.93 19.07a9.999 9.999 0 0 1 0-14.142l1.414 1.414A8 8 0 0 0 4.61 8.938ZM20 12a7.999 7.999 0 0 0-2.343-5.657l1.414-1.414a10 10 0 0 1 0 14.142l-1.414-1.414A7.999 7.999 0 0 0 20 12Z"
            />
            <path
                fill="currentColor"
                fillRule="evenodd"
                d="M12 18a6 6 0 1 0 0-12 6 6 0 0 0 0 12Zm0-2a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z"
                clipRule="evenodd"
            />
        </svg>
    );
}
