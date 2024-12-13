/**
 * SWIZZLED VERSION: 3.5.2
 * REASONS:
 *  - Add check for reference CodeBlock so it can coexist with the original CodeBlock and the live code block
 */
import React from 'react';
import CodeBlock from '@theme-original/CodeBlock';
import ReferenceCodeBlock from '@saucelabs/theme-github-codeblock/build/theme/ReferenceCodeBlock';
import { ReferenceCodeBlockProps } from '@saucelabs/theme-github-codeblock/build/theme/types';
import type CodeBlockType from '@theme/CodeBlock';
import type {WrapperProps} from '@docusaurus/types';

type Props = WrapperProps<typeof CodeBlockType>;

function isReferenceCodeBlockType(props: object): props is ReferenceCodeBlockProps {
  return 'reference' in props 
    || ('metastring' in props && typeof props.metastring === 'string' && props.metastring.split(' ').includes('reference'));
}

// Wrap CodeBlock to check if it is a reference (saucepans) CodeBlock.
// If it isn't, we just return the live plugin CodeBlock which will check,
// if the CodeBlock is a live CodeBlock or the original CodeBlock
export default function CodeBlockWrapper(props: ReferenceCodeBlockProps | Props): JSX.Element {
  return (
    <>
      {isReferenceCodeBlockType(props) ?  <ReferenceCodeBlock {...props} /> :  <CodeBlock {...props} />}
    </>
  );
}
