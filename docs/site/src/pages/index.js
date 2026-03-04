// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from "react";

import Layout from "@theme/Layout";
import Link from "@docusaurus/Link";
import styles from "./index.module.css";

const ArrowIcon = () => (
  <svg
    width="16"
    height="16"
    viewBox="0 0 16 16"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    className={styles.CardLinkArrow}
  >
    <path
      d="M4 12L12 4M12 4H5.6M12 4V10.4"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

export default function Home() {
const HomeCard = (props) => {
    const { title, children } = props;
    return (
    <div className={`p-px w-full`}>
        <div className={styles.Card}>
          {title && (
            <Link to="#" className={styles.CardTitle}> 
              {title}
            </Link>
          )}
          <div className={styles.CardLinksContainer}>{children}</div>
        </div>
      </div>
    );
  };
const HomeCardCTA = () => {
    return (
      <div className={`p-px w-full`}>
        <div className={styles.CardCTA}>
          <h3 className={styles.CardCTATitle}>
            Build your dApp on IOTA
          </h3>
          <Link
            className={styles.ctaButton}
            to="/developer/getting-started"
          >
            Start now
          </Link>
        </div>
      </div>
    );
  };

  return (
    <Layout
      style={{
        background: "var(--iota-black)",
      }}
    >
      {" "}
      <div className="dark:bg-iota-black overflow-hidden">
       <div className={styles.HeroContainer}>
          <div className={styles.HeroText}>
            <h1 className="text-5xl center-text text-black dark:text-white">
              IOTA Documentation
            </h1>
            <h2
              className="text-xl text-gray-600 center-text dark:text-gray-400"
            >
              Discover the power of IOTA through examples, guides, and
              explanations.
            </h2>
          </div>
        </div>
      
        <div className={styles.CardGrid}>
          <HomeCard title="About IOTA">
            <Link className={styles.CardLink} to="./about-iota/iota-architecture">
              IOTA Architecture <ArrowIcon />
            </Link>
            <Link className={styles.CardLink} to="./about-iota/tokenomics">
              Tokenomics <ArrowIcon />
            </Link>
            <Link className={styles.CardLink} to="./developer/cryptography">
              Cryptography <ArrowIcon />
            </Link>
            <Link className={styles.CardLink} to="./developer/standards">
              Standards <ArrowIcon />

            </Link>
          </HomeCard>
          <HomeCard title="Developers">
            <Link className={styles.CardLink} to="./developer/getting-started"> 
              Getting started <ArrowIcon /> 
            </Link>
            <Link className={styles.CardLink} to="./developer/iota-101"> 
              IOTA Developer Basics <ArrowIcon /> 
            </Link>
            <Link className={styles.CardLink} to="./developer/iota-101/move-overview/"> 
              Move <ArrowIcon /> 
            </Link>
          </HomeCard>
          <HomeCard title="Validators & Node operators"> 
            <Link className={styles.CardLink} to="./operator/validator-node/configuration"> 
              Validator configuration <ArrowIcon /> 
            </Link>
            <Link className={styles.CardLink} to="./operator/full-node/overview"> 
              Run an IOTA Full node <ArrowIcon /> 
             
            </Link>
          </HomeCard>
          <HomeCard title="References">
            <Link className={styles.CardLink} to="/developer/ts-sdk/dapp-kit/"> 
              IOTA dApp Kit <ArrowIcon />
            </Link>
            <Link className={styles.CardLink} to="/developer/references/iota-api"> 
              IOTA API <ArrowIcon /> 
            </Link>
            <Link className={styles.CardLink} to="https://github.com/iotaledger/iota/tree/develop/crates/iota-framework/docs"> 
              IOTA framework (GitHub) <ArrowIcon />
            </Link>
            <Link className={styles.CardLink} to="https://github.com/iotaledger/iota/tree/develop/crates/iota-sdk"> 
              Rust SDK (GitHub) <ArrowIcon /> 
            </Link>
          </HomeCard>
          <HomeCard title="Resources"> 
            <Link className={styles.CardLink} to="https://iotalabs.io/projects"> 
              IOTA ecosystem directory <ArrowIcon /> 
            </Link>
            <Link className={styles.CardLink} to="https://blog.iota.org/"> 
              IOTA blog <ArrowIcon /> 
            </Link>
            <Link className={styles.CardLink} to="developer/dev-cheat-sheet"> 
              IOTA dev cheat sheet <ArrowIcon /> 
            </Link>
          </HomeCard>
          <HomeCardCTA />
        </div>

        <div className={styles.sectionHeader}>
          <h2 className="h1 font-twkeverett">Why IOTA?</h2>
          <h3 className="h3 text-center">
            IOTA is the first internet-scale programmable blockchain platform
          </h3>
        </div>
        <div className={styles.why}>
          <div className={styles.whyImgCard}>
            <img height={"90%"} src="/img/index/blocks.png" alt="Decorative visual" />
          </div>
          <div className={styles.cardsB}>
            <div className={styles.cardB}>
              <svg
                width="32"
                height="32"
                viewBox="0 0 32 32"
                fill="none"
                xmlns="http://www.w3.org/2000/svg"
              >
                <path
                  d="M17.3337 3.99902V13.3324H25.3337L14.667 27.999V18.6657H6.66699L17.3337 3.99902Z"
                  className="dark:stroke-[#C0DEFF]"
                  stroke="black"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
              <span>Unmatched scalability, instant settlement</span>
            </div>
            <div className={styles.cardB}>
              <svg
                width="32"
                height="32"
                viewBox="0 0 32 32"
                fill="none"
                xmlns="http://www.w3.org/2000/svg"
              >
                <path
                  d="M12.5664 12H15.5996"
                  className="dark:stroke-[#C0DEFF]"
                  stroke="black"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
                <path
                  d="M12.5664 17.333H22.5171"
                  className="dark:stroke-[#C0DEFF]"
                  stroke="black"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
                <path
                  d="M12.5664 22.667H22.5171"
                  className="dark:stroke-[#C0DEFF]"
                  stroke="black"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
                <rect
                  x="8.76855"
                  y="3.67871"
                  width="20.6312"
                  height="24.6722"
                  rx="2"
                  className="dark:stroke-[#C0DEFF]"
                  stroke="black"
                  strokeWidth="2"
                />
                <path
                  d="M8.60445 17.6113L3.21655 17.6113C3.09911 17.6113 3.00391 17.7065 3.00391 17.824V25.4746C3.00391 27.0627 4.29131 28.3501 5.87941 28.3501V28.3501C7.46751 28.3501 8.75492 27.0627 8.75492 25.4746V23.1274"
                  className="dark:stroke-[#C0DEFF]"
                  stroke="black"
                  strokeWidth="2"
                  strokeLinecap="round"
                />
                <path
                  d="M6.20703 28.3496H13.3685"
                  className="dark:stroke-[#C0DEFF]"
                  stroke="black"
                  strokeWidth="2"
                  strokeLinecap="round"
                />
              </svg>

              <span>
                A safe smart contract language accessible to mainstream
                developers
              </span>
            </div>
            <div className={styles.cardB}>
              <svg
                width="32"
                height="32"
                viewBox="0 0 32 32"
                fill="none"
                xmlns="http://www.w3.org/2000/svg"
              >
                <path
                  d="M16 20.0007H7.33333C6.44928 20.0007 5.60143 19.6495 4.97631 19.0243C4.35119 18.3992 4 17.5514 4 16.6673C4 15.7833 4.35119 14.9354 4.97631 14.3103C5.60143 13.6852 6.44928 13.334 7.33333 13.334H8"
                  className="dark:stroke-[#C0DEFF]"
                  stroke="black"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
                <path
                  d="M20.0002 16V24.6667C20.0002 25.5507 19.649 26.3986 19.0239 27.0237C18.3987 27.6488 17.5509 28 16.6668 28C15.7828 28 14.9349 27.6488 14.3098 27.0237C13.6847 26.3986 13.3335 25.5507 13.3335 24.6667V24"
                  className="dark:stroke-[#C0DEFF]"
                  stroke="black"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
                <path
                  d="M16 12H24.6667C25.5507 12 26.3986 12.3512 27.0237 12.9763C27.6488 13.6014 28 14.4493 28 15.3333C28 16.2174 27.6488 17.0652 27.0237 17.6904C26.3986 18.3155 25.5507 18.6667 24.6667 18.6667H24"
                  className="dark:stroke-[#C0DEFF]"
                  stroke="black"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
                <path
                  d="M12 16V7.33333C12 6.44928 12.3512 5.60143 12.9763 4.97631C13.6014 4.35119 14.4493 4 15.3333 4C16.2174 4 17.0652 4.35119 17.6904 4.97631C18.3155 5.60143 18.6667 6.44928 18.6667 7.33333V8"
                  className="dark:stroke-[#C0DEFF]"
                  stroke="black"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>

              <span>Ability to define rich and composable on-chain assets</span>
            </div>
            <div className={styles.cardB}>
              <svg
                width="32"
                height="33"
                viewBox="0 0 32 33"
                fill="none"
                xmlns="http://www.w3.org/2000/svg"
              >
                <rect
                  x="4.21191"
                  y="5.48926"
                  width="9.42373"
                  height="9.42373"
                  rx="2"
                  strokeWidth="2"
                  className="dark:stroke-[#C0DEFF]"
                  stroke="black"
                />
                <rect
                  x="16.4422"
                  y="8.47931"
                  width="9.42373"
                  height="9.42373"
                  rx="2"
                  transform="rotate(-30 16.4422 8.47931)"
                  strokeWidth="2"
                  className="dark:stroke-[#C0DEFF]"
                  stroke="black"
                />
                <rect
                  x="4.21191"
                  y="19.4453"
                  width="9.42373"
                  height="9.42373"
                  rx="2"
                  strokeWidth="2"
                  className="dark:stroke-[#C0DEFF]"
                  stroke="black"
                />
                <rect
                  x="18.166"
                  y="19.4453"
                  width="9.42373"
                  height="9.42373"
                  rx="2"
                  strokeWidth="2"
                  className="dark:stroke-[#C0DEFF]"
                  stroke="black"
                />
              </svg>

              <span>Better user experience for web3 apps</span>
            </div>
          </div>
        </div>
        <div className={styles.TwoColParagraph}>
          <div className={styles.TwoColItem}>
            <span>Scalability</span>
            <p>
              IOTA scales horizontally to meet the demands of applications.
              Network capacity grows in proportion to the increase in IOTA
              validators' processing power by adding workers, resulting in low
              gas fees even during high network traffic. This scalability
              characteristic is in sharp contrast to other blockchains with
              rigid bottlenecks.
            </p>
          </div>
          <div className={styles.TwoColItem}>
            <span>Move</span>
            <p>
              Move design prevents issues such as reentrancy vulnerabilities,
              poison tokens, and spoofed token approvals that attackers have
              leveraged to steal millions on other platforms. The emphasis on
              safety and expressivity provides a more straightforward transition
              from web 2.0 to web3 for developers, without the need to
              understand the intricacies of the underlying infrastructure.
            </p>
          </div>
          <div className={styles.TwoColItem}>
            <span>On-chain assets</span>
            <p>
              Rich on-chain assets enable new applications and economies based
              on utility without relying solely on artificial scarcity.
              Developers can implement dynamic NFTs that you can upgrade,
              bundle, and group in an application-specific manner, such as
              changes in avatars and customizable items based on gameplay. This
              capability delivers stronger in-game economies as NFT behavior
              gets fully reflected on-chain, making NFTs more valuable and
              delivering more engaging feedback loops.
            </p>
          </div>
          <div className={styles.TwoColItem}>
            <span>Built for Web3</span>
            <p>
              IOTA aims to be the most accessible smart contract platform,
              empowering developers to create great user experiences in web3. To
              usher in the next billion users, IOTA empowers developers with
              various tools to take advantage of the power of the IOTA
              blockchain. The IOTA Development Kit (SDK) will enable developers
              to build without boundaries.
            </p>
          </div>
        </div>
      </div>
    </Layout>
  );
}
