import React, { useState } from 'react';
import { ChainId } from '../ChainId';
import { NetworkProps, MoveProps } from '../constant';
import CodeBlock from '@theme/CodeBlock';
import Admonition from '@theme/Admonition';

// L1 component
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
        {props.faucetUrl && (
          <tr>
            <th>Faucet</th>
            <td>
              <CodeBlock>
                {props.faucetUrl}
              </CodeBlock>
            </td>
          </tr>
        )}
      </tbody>
    </table>
  );
}

// Testnet Component
function Testnet(props: NetworkProps) {
  return (
    <table>
      <tbody>
        <tr>
          <th>Base Token</th>
          <td>{props.baseToken}</td>
        </tr>
        {props.protocol &&<tr>
          <th>Protocol</th>
          <td>{props.protocol}</td>
        </tr>}
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
        {props.faucetUrl && (
          <tr>
            <th>Faucet</th>
            <td>
              <CodeBlock>
                {props.faucetUrl}
              </CodeBlock>
            </td>
          </tr>
        )}
      </tbody>
    </table>
  );
}

// Devtnet Component
function Devnet(props: NetworkProps) {
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
        {props.faucetUrl && (
          <tr>
            <th>Faucet</th>
            <td>
              <CodeBlock>
                {props.faucetUrl}
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
        {props.evmCustom.ankrApiUrls && (
          <tr>
            <th>
              <Admonition type='tip' title='Ankr API URLs'>
                <a href={'/build/rpcProviders/'}>Ankr API</a> enterprise-grade
                globally distributed endpoints
              </Admonition>
            </th>
            <td>
              {props.evmCustom.ankrApiUrls.map((object, index) =>
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

// EvmCustom component
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

// Move component
function Move(props: MoveProps) {
  return (
    <table>
      <tbody>
        <tr>
          <th>Base Token</th>
          <td>{props.baseToken}</td>
        </tr>
        {props.explorerUrl && (
          <tr>
            <th scope="row">Explorer URL</th>
            <td>
              <CodeBlock>{props.explorerUrl}</CodeBlock>
            </td>
          </tr>
        )}
        <tr>
          <th>JSON RPC URL</th>
          <td>
            <CodeBlock>{props.jsonRpcUrl}</CodeBlock>
          </td>
        </tr>
        <tr>
          <th>Indexer RPC</th>
          <td>
            <CodeBlock>{props.indexerRpc}</CodeBlock>
          </td>
        </tr>
        <tr>
          <th>GraphQL RPC</th>
          <td>
            <CodeBlock>{props.graphqlRpc}</CodeBlock>
          </td>
        </tr>
        <tr>
          <th>RPC Websocket URL</th>
          <td>
            <CodeBlock>{props.jsonRpcWebsocketUrl}</CodeBlock>
          </td>
        </tr>
        {props.faucetUrl && (
          <tr>
            <th>Faucet URL</th>
            <td>
              <CodeBlock>
                {props.faucetUrl}
              </CodeBlock>
            </td>
          </tr>
        )}
      </tbody>
    </table>
  );
}

export default {
  L1,
  Evm,
  EvmCustom,
  Move,
  Testnet,
  Devnet
};
