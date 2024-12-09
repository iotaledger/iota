/**
 * SWIZZLED VERSION: 3.5.2
 * REASONS:
 *  - Add default components
 *  - Add FeedbackForm component
 */
import React from 'react';
import { MDXProvider } from '@mdx-js/react';
import MDXComponents from '@theme/MDXComponents';
import type { Props } from '@theme/MDXContent';
import { Card, Cards } from "@site/src/components/Cards";
import Tabs from "@theme/Tabs";
import TabItem from "@theme/TabItem";
import FeedbackForm from "@site/src/components/FeedbackForm"; 

export default function MDXContent({ children }: Props): JSX.Element {
  const components = {
    ...MDXComponents,
    Card,
    Cards,
    Tabs,
    TabItem,
  };
  return (
    <MDXProvider components={components}>
      {children}
      <FeedbackForm /> 
    </MDXProvider>
  );
}
