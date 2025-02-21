// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function ExternalImage({
    ...imageProps
}: React.ImgHTMLAttributes<HTMLImageElement>): JSX.Element {
    // eslint-disable-next-line @next/next/no-img-element
    return <img {...imageProps} alt={imageProps.alt} />;
}
