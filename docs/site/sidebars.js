// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//const why_iota = require("../content/sidebars/why_iota.js");
const developer = require("../content/sidebars/developer.js");
const aboutIota = require("../content/sidebars/about-iota.js");
const operator = require("../content/sidebars/operator.js");
const references = require("../content/sidebars/references.js");
const tsSDK = require("../content/sidebars/ts-sdk.js")
const identity = require("../content/sidebars/ts-sdk.js")

const sidebars = {
  //whyIOTASidebar: why_iota,
  developerSidebar: developer,
  operatorSidebar: operator,
  aboutIotaSidebar: aboutIota,
  referencesSidebar: references,
  tsSDK: tsSDK,
  identity: identity,
};

module.exports = sidebars;
