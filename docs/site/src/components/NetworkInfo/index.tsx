import React, { useState } from 'react';
import { ChainId } from '../ChainId';
import { NetworkProps } from '../constant';
import CodeBlock from '@theme/CodeBlock';
import Admonition from '@theme/Admonition';

// Create an explorer link with the networks as query and slook for an object if provided
const buildHref = (explorer, hasQuery: boolean, objectId: string | undefined = null,) =>
  hasQuery
    ? objectId
      ? `${(explorer as { url: string; query: string }).url}/object/${objectId}${(explorer as { url: string; query: string }).query}`
      : `${(explorer as { url: string; query: string }).url}${(explorer as { url: string; query: string }).query}`
    : objectId
      ? `${explorer}/object/${objectId}`
    : `${explorer}`;

// L1 component
function L1(props: NetworkProps) {
  const hasQuery = typeof props.explorer !== 'string'
  const href = buildHref(props.explorer, hasQuery);

  return (
    <table>
      <tbody>
        <tr>
          <th>Base Token</th>
          <td>{props.baseToken}</td>
        </tr>
        <tr>
          <th>Protocol</th>
          <td>{props.protocol}</td>
        </tr>
        <tr>
          <th>Explorer</th>
          <td>{href}</td>
        </tr>
        <tr>
          <th>JSON RPC URL</th>
            <td>
              <CodeBlock>{props.rpc.json.core}</CodeBlock>
              <tr>
                <th>Websocket</th>
                <td>
                  <CodeBlock>{props.rpc.json.websocket}</CodeBlock>
                </td>
              </tr>
              <tr>
                <th>Indexer</th>
                <td>
                  <CodeBlock>{props.rpc.json.indexer}</CodeBlock>
                </td>
              </tr>
          </td>
        </tr>
        <tr>
          <th>GraphQL RPC URL</th>
          <td>
            <CodeBlock>{props.rpc.graphql}</CodeBlock>
          </td>
        </tr>
        {props.faucet && (
          <tr>
            <th>Faucet</th>
            <td>
              <CodeBlock>
                {props.faucet}
              </CodeBlock>
            </td>
          </tr>
        )}
      </tbody>
    </table>
  );
}

// EVM component
function Evm(props: NetworkProps) {
  return (
    <table>
      <tbody>
        <tr>
          <th>Base Token</th>
          <td>{props.baseToken}</td>
        </tr>
        <tr>
          <th>Protocol</th>
          <td>ISC / EVM</td>
        </tr>
        <tr>
          <th>Chain ID</th>
          <td>
            <ChainId url={props.evm?.rpcUrls?.[0]} />
          </td>
        </tr>
        <tr>
          <th>RPC URL</th>
          <td>
            {props.evm?.rpcUrls?.map((url, index) => (
              <CodeBlock key={index}>{url}</CodeBlock>
            ))}
          </td>
        </tr>
        {props.evmCustom?.ankrApiUrls && (
          <tr>
            <th>
              <Admonition type='tip' title='Ankr API URLs'>
                <a href={'/build/rpcProviders/'}>Ankr API</a> enterprise-grade
                globally distributed endpoints
              </Admonition>
            </th>
            <td>
              {props.evmCustom?.ankrApiUrls.map((object, index) =>
                typeof object === 'string' ? (
                  <CodeBlock key={index}> {object as string} </CodeBlock>
                ) : (
                  <CodeBlock title={Object.keys(object)[0]} key={index}>
                    {' '}
                    {Object.values(object)[0]}{' '}
                  </CodeBlock>
                ),
              )}
            </td>
          </tr>
        )}
        <tr>
          <th>Explorer</th>
          <td>
            <a
              href={props.evm?.blockExplorerUrls?.[0]}
              target='_blank'
              rel='noopener noreferrer'
            >
              {props.evm?.blockExplorerUrls?.[0]}
            </a>
          </td>
        </tr>
        <tr>
          <th>
            {props.evmCustom?.bridge?.hasFaucet ? 'Toolkit & Faucet' : 'Toolkit'}
          </th>
          <td>
            <a
              href={props.evmCustom?.bridge?.url}
              target='_blank'
              rel='noopener noreferrer'
            >
              {props.evmCustom?.bridge?.url}
            </a>
          </td>
        </tr>
        {props.evmCustom?.api && (
          <tr>
            <th>WASP API</th>
            <td>
              <CodeBlock> {props.evmCustom.api} </CodeBlock>
            </td>
          </tr>
        )}
      </tbody>
    </table>
  );
}

// EvmCustom component
function EvmCustom(props: NetworkProps) {
  const hasQuery = typeof props.explorer !== 'string'

  return (
    <table>
      <tbody>
        <tr>
          <th>Chain ID</th>
          <td>
            <a
              href={buildHref(props.explorer, hasQuery, props.evmCustom?.chainId)}
              target='_blank'
              rel='noopener noreferrer'
            >
              {props.evmCustom?.chainId}
            </a>
          </td>
        </tr>
        <tr>
          <th>Package ID</th>
          <td>
            <a
            href={buildHref(props.explorer, hasQuery, props.evmCustom?.packageId)}
              target='_blank'
              rel='noopener noreferrer'
            >
              {props.evmCustom?.packageId}
            </a>
          </td>
        </tr>
      </tbody>
    </table>
  );
}

export default {
  L1,
  Evm,
  EvmCustom,
};
