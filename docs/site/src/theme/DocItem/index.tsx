/**
 * SWIZZLED VERSION: 3.5.2
 * REASONS:
 *  - Add option to allow custom components before the DocItem
 */
import React from 'react';
import DocItem from '@theme-original/DocItem';
import type DocItemType from '@theme/DocItem';
import type {WrapperProps} from '@docusaurus/types';
import { useLocation } from '@docusaurus/router';

import preMDXComponents from '../../../configs/preContent';

type Props = WrapperProps<typeof DocItemType>;

export default function DocItemWrapper(props: Props): JSX.Element {
  const { pathname } = useLocation();
  const matchingKey = Object.keys(preMDXComponents).find((key) => new RegExp(key).test(pathname));
  
  return (
    <>
      {preMDXComponents[matchingKey]}
      <DocItem {...props} />
    </>
  );
}
