/**
 * SWIZZLED VERSION: 3.5.2
 * REASONS:
 *  - Adapt to custom IOTA design including social media icons
 */
import React from 'react';
import clsx from 'clsx';
import type {Props} from '@theme/Footer/Layout';
import Social from '@site/src/components/Social';

export default function FooterLayout({
  style,
  links,
  logo,
  copyright,
}: Props): JSX.Element {
  return (
    <footer
      className={clsx('footer', {
        'footer--dark': style === 'dark',
      })}>
      <div className="container container-fluid">
        {links}
        {(logo || copyright) && (
          <div className="footer__bottom text--left">
            <div className="row">
              {logo && <div className="col col--2 col--offset-1 footer__col margin-bottom--sm">{logo}</div>}
              {copyright && <div className="col col--6 footer__col text-sm margin-top--md">{copyright}</div>}
              <div className='col footer__col margin-top--md'>
                <Social />
              </div>
            </div>
          </div>
        )}
      </div>
    </footer>
  );
}
