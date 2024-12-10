/**
 * SWIZZLED VERSION: 3.5.2
 * REASONS:
 *  - Add check for reference CodeBlock so it can coexist with the original CodeBlock and the live code block
 */
import React from 'react';
import CodeBlock from '@theme-original/CodeBlock';
import ReferenceCodeBlock from '@saucelabs/theme-github-codeblock/build/theme/ReferenceCodeBlock';
import type CodeBlockType from '@theme/CodeBlock';
import type {WrapperProps} from '@docusaurus/types';

type Props = WrapperProps<typeof CodeBlockType>;

export default function CodeBlockWrapper(props: Props): JSX.Element {
  if (props.reference || props.metastring?.split(' ').includes('reference')) {
    return (
      <>
        <ReferenceCodeBlock {...props} />
      </>
    );
  } else {
    return (
      <>
        <CodeBlock {...props} />
      </>
    );
  }
}
