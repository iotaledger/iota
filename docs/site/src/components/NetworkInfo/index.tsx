import React from 'react';
import { ChainId } from '../ChainId';
import { NetworkProps } from '../constant';
import CodeBlock from '@theme/CodeBlock';
import Admonition from '@theme/Admonition';

function L1(props: NetworkProps) {
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
          <th>HTTP REST API</th>
          <td>
            <CodeBlock>{props.httpRestApi}</CodeBlock>
          </td>
        </tr>
        <tr>
          <th>Event API</th>
          <td>
            <CodeBlock>{props.eventApi}</CodeBlock>
          </td>
        </tr>
        <tr>
          <th>Permanode API</th>
          <td>
            <CodeBlock>{props.permaNodeApi}</CodeBlock>
          </td>
        </tr>
        {props.faucet && (
          <tr>
            <th>Faucet</th>
            <td>
              <a href={props.faucet} target='_blank' rel='noopener noreferrer'>
                {props.faucet}
              </a>
            </td>
          </tr>
        )}
      </tbody>
    </table>
  );
}

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
            <ChainId url={props.evm.rpcUrls[0]} />
          </td>
        </tr>
        <tr>
          <th>RPC URL</th>
          <td>
            {props.evm.rpcUrls.map((url, index) => (
              <CodeBlock key={index}>{url}</CodeBlock>
            ))}
          </td>
        </tr>
        {props.evmCustom.blastApiUrls && (
          <tr>
            <th>
              <Admonition type='tip' title='Blast API URLs'>
                <a href={'/build/blastAPI/'}>Blast API</a> provides highly
                scalable fault-tolerant API endpoints.
              </Admonition>
            </th>
            <td>
              {props.evmCustom.blastApiUrls.map((object, index) =>
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
              href={props.evm.blockExplorerUrls[0]}
              target='_blank'
              rel='noopener noreferrer'
            >
              {props.evm.blockExplorerUrls[0]}
            </a>
          </td>
        </tr>
        <tr>
          <th>
            {props.evmCustom.toolkit.hasFaucet ? 'Toolkit & Faucet' : 'Toolkit'}
          </th>
          <td>
            <a
              href={props.evmCustom.toolkit.url}
              target='_blank'
              rel='noopener noreferrer'
            >
              {props.evmCustom.toolkit.url}
            </a>
          </td>
        </tr>
        <tr>
          <th>WASP API</th>
          <td>
            <CodeBlock> {props.evmCustom.api} </CodeBlock>
          </td>
        </tr>
      </tbody>
    </table>
  );
}

function EvmCustom(props: NetworkProps) {
  return (
    <table>
      <tbody>
        <tr>
          <th>Chain Address</th>
          <td>
            <a
              href={props.explorer + '/addr/' + props.evmCustom.chainAddress}
              target='_blank'
              rel='noopener noreferrer'
            >
              {props.evmCustom.chainAddress}
            </a>
          </td>
        </tr>
        <tr>
          <th>Alias ID</th>
          <td>{props.evmCustom.aliasId}</td>
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
