// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { ImageShape } from './card.enums';

export interface ImagePlaceholderProps {
    /**
     * The variant of the image placeholder.
     */
    variant?: ImageShape;
}

export function CardImagePlaceholder({ variant = ImageShape.Rounded }: ImagePlaceholderProps) {
    if (variant === ImageShape.Rounded) {
        return <ImagePlaceholderRounded />;
    }

    return <ImagePlaceholderSquareRounded />;
}

function ImagePlaceholderRounded() {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="40"
            height="40"
            viewBox="0 0 40 40"
            fill="none"
        >
            <g clip-path="url(#clip0_880_55185)">
                <path
                    d="M0 20C0 8.95431 8.95431 0 20 0C31.0457 0 40 8.95431 40 20C40 31.0457 31.0457 40 20 40C8.95431 40 0 31.0457 0 20Z"
                    fill="#5B9CFE"
                />
                <path
                    d="M0 20C0 8.95431 8.95431 0 20 0C31.0457 0 40 8.95431 40 20C40 31.0457 31.0457 40 20 40C8.95431 40 0 31.0457 0 20Z"
                    fill="url(#paint0_linear_880_55185)"
                    fill-opacity="0.2"
                />
                <path
                    d="M0 20C0 8.95431 8.95431 0 20 0C31.0457 0 40 8.95431 40 20C40 31.0457 31.0457 40 20 40C8.95431 40 0 31.0457 0 20Z"
                    fill="url(#paint1_linear_880_55185)"
                    fill-opacity="0.2"
                />
                <path
                    d="M0.5 20C0.5 9.23045 9.23045 0.5 20 0.5C30.7696 0.5 39.5 9.23045 39.5 20C39.5 30.7696 30.7696 39.5 20 39.5C9.23045 39.5 0.5 30.7696 0.5 20Z"
                    stroke="#002F6D"
                    stroke-opacity="0.08"
                />
            </g>
            <defs>
                <linearGradient
                    id="paint0_linear_880_55185"
                    x1="20"
                    y1="0"
                    x2="29.5238"
                    y2="22.381"
                    gradientUnits="userSpaceOnUse"
                >
                    <stop offset="0.990135" stop-opacity="0" />
                    <stop offset="1" />
                </linearGradient>
                <linearGradient
                    id="paint1_linear_880_55185"
                    x1="20"
                    y1="0"
                    x2="11.4286"
                    y2="9.04762"
                    gradientUnits="userSpaceOnUse"
                >
                    <stop offset="0.9999" stop-opacity="0" />
                    <stop offset="1" />
                </linearGradient>
                <clipPath id="clip0_880_55185">
                    <path
                        d="M0 20C0 8.95431 8.95431 0 20 0C31.0457 0 40 8.95431 40 20C40 31.0457 31.0457 40 20 40C8.95431 40 0 31.0457 0 20Z"
                        fill="white"
                    />
                </clipPath>
            </defs>
        </svg>
    );
}

function ImagePlaceholderSquareRounded() {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="40"
            height="40"
            viewBox="0 0 40 40"
            fill="none"
        >
            <g clip-path="url(#clip0_880_55190)">
                <path
                    d="M0 8C0 3.58172 3.58172 0 8 0H32C36.4183 0 40 3.58172 40 8V32C40 36.4183 36.4183 40 32 40H8C3.58172 40 0 36.4183 0 32V8Z"
                    fill="#5B9CFE"
                />
                <path
                    d="M0 8C0 3.58172 3.58172 0 8 0H32C36.4183 0 40 3.58172 40 8V32C40 36.4183 36.4183 40 32 40H8C3.58172 40 0 36.4183 0 32V8Z"
                    fill="url(#paint0_linear_880_55190)"
                    fill-opacity="0.2"
                />
                <path
                    d="M0 8C0 3.58172 3.58172 0 8 0H32C36.4183 0 40 3.58172 40 8V32C40 36.4183 36.4183 40 32 40H8C3.58172 40 0 36.4183 0 32V8Z"
                    fill="url(#paint1_linear_880_55190)"
                    fill-opacity="0.2"
                />
                <path
                    d="M0.5 8C0.5 3.85786 3.85786 0.5 8 0.5H32C36.1421 0.5 39.5 3.85786 39.5 8V32C39.5 36.1421 36.1421 39.5 32 39.5H8C3.85786 39.5 0.5 36.1421 0.5 32V8Z"
                    stroke="#002F6D"
                    stroke-opacity="0.08"
                />
            </g>
            <defs>
                <linearGradient
                    id="paint0_linear_880_55190"
                    x1="20"
                    y1="0"
                    x2="29.5238"
                    y2="22.381"
                    gradientUnits="userSpaceOnUse"
                >
                    <stop offset="0.990135" stop-opacity="0" />
                    <stop offset="1" />
                </linearGradient>
                <linearGradient
                    id="paint1_linear_880_55190"
                    x1="20"
                    y1="0"
                    x2="11.4286"
                    y2="9.04762"
                    gradientUnits="userSpaceOnUse"
                >
                    <stop offset="0.9999" stop-opacity="0" />
                    <stop offset="1" />
                </linearGradient>
                <clipPath id="clip0_880_55190">
                    <path
                        d="M0 8C0 3.58172 3.58172 0 8 0H32C36.4183 0 40 3.58172 40 8V32C40 36.4183 36.4183 40 32 40H8C3.58172 40 0 36.4183 0 32V8Z"
                        fill="white"
                    />
                </clipPath>
            </defs>
        </svg>
    );
}
