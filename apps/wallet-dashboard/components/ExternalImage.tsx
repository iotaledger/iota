// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export default function ExternalImage({
    ...imageProps
}: React.ImgHTMLAttributes<HTMLImageElement>): JSX.Element {
    return <img {...imageProps} />;
}
