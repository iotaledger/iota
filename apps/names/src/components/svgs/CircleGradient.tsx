// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function SvgCircleGradient(props: React.SVGProps<SVGSVGElement>): React.JSX.Element {
    return (
        <svg
            width="524"
            height="524"
            viewBox="0 0 524 524"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
            {...props}
        >
            <g opacity="0.2" filter="url(#filter0_f_10813_6848)">
                <circle cx="262" cy="262" r="102" fill="url(#paint0_linear_10813_6848)" />
            </g>
            <defs>
                <filter
                    id="filter0_f_10813_6848"
                    x="0"
                    y="0"
                    width="524"
                    height="524"
                    filterUnits="userSpaceOnUse"
                    colorInterpolationFilters="sRGB"
                >
                    <feFlood floodOpacity="0" result="BackgroundImageFix" />
                    <feBlend
                        mode="normal"
                        in="SourceGraphic"
                        in2="BackgroundImageFix"
                        result="shape"
                    />
                    <feGaussianBlur stdDeviation="80" result="effect1_foregroundBlur_10813_6848" />
                </filter>
                <linearGradient
                    id="paint0_linear_10813_6848"
                    x1="364"
                    y1="160"
                    x2="168.088"
                    y2="371.321"
                    gradientUnits="userSpaceOnUse"
                >
                    <stop stopColor="#A139FF" />
                    <stop offset="0.5" stopColor="#3131FF" />
                    <stop offset="1" stopColor="#14F0D6" />
                </linearGradient>
            </defs>
        </svg>
    );
}
