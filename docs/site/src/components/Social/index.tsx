import React from 'react';
import { useThemeConfig } from '@docusaurus/theme-common';
import './styles.css';
import get_socials_data from '@site/src/utils/socials';
import { ThemeConfig } from '@docusaurus/preset-classic';
import Link from '@docusaurus/Link';

export interface Social {
  url: string;
}

export interface SocialsConfig extends ThemeConfig {
  socials: string[];
}

function SocialLink({url}: Social) {
  const { Icon } = get_socials_data(url);

  return (
    <Link
      className='padding-horiz--sm padding-vert--md'
      to={url}
    >
      <Icon className='social__icon' />
    </Link>
  );
}

function Social() {
  const { socials } = useThemeConfig() as SocialsConfig;

  return (
    <div>
      {socials &&
        socials.map((social, idx) => <SocialLink key={idx} url={social} />)}
    </div>
  );
}

export default Social;
