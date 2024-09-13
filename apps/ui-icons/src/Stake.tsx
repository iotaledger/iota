// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgStake(props: SVGProps<SVGSVGElement>) {
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
                d="M12.001 6.803a6 6 0 1 1 0 10.394 6 6 0 1 1 0-10.393Zm2.219 1.274a4 4 0 1 1-.553 7.694A5.975 5.975 0 0 0 15 12c0-1.429-.5-2.74-1.333-3.771.18-.064.364-.115.553-.152ZM13 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0Z"
                clipRule="evenodd"
            />
        </svg>
    );
}
