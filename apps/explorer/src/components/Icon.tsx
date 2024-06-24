// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

interface IconProps {
    icon: React.FunctionComponent<React.SVGProps<SVGSVGElement>>;
    width?: string;
    height?: string;
    fill?: string;
}

function Icon({
    icon,
    width = '24px',
    height = '24px',
    fill = 'currentColor',
}: IconProps): JSX.Element {
    const IconComponent = icon;

    return <IconComponent width={width} height={height} fill={fill} />;
}

export default Icon;
