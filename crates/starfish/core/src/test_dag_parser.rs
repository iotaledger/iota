// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    char,
    collections::{HashMap, HashSet},
    sync::Arc,
};

use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_until, take_while_m_n, take_while1},
    character::complete::{
        alpha1, anychar, char, digit1, multispace0, multispace1, space0, space1,
    },
    combinator::{map, map_res, opt},
    multi::{many0, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, terminated, tuple},
};
use nom::bytes::complete::take_while;
use starfish_config::AuthorityIndex;

use crate::{
    block_header::{BlockRef, Round, Slot},
    context::Context,
    test_dag_builder::{ConnectionSpec, DagBuilder},
};

/// DagParser
///
/// Usage:
///
/// ```
/// let dag_str = "DAG {
///     Round 0 : { 4 },
///     Round 1 : { * },
///     Round 2 : { * },
///     Round 3 : { * },
///     Round 4 : {
///         A -> [-D3],
///         B -> [*],
///         C -> [*],
///         D -> [*],
///     },
///     Round 5 : {
///         A -> [*],
///         B -> [*],
///         C -> [A4],
///         D -> [A4],
///     },
///     Round 6 : { * },
///     Round 7 : { * },
///     Round 8 : { * },
///     }";
///
/// let (_, dag_builder) = parse_dag(dag_str).expect("Invalid dag"); // parse DAG DSL
/// dag_builder.print(); // print the parsed DAG
/// dag_builder.persist_all_blocks(dag_state.clone()); // persist all blocks to DagState
/// ```
pub(crate) fn parse_dag(dag_string: &str) -> Result<DagBuilder, nom::Err<()>> {
    let (input, _) = tuple((tag("DAG"), multispace0, char('{')))(dag_string)?;

    let (mut input, num_authors) = parse_genesis(input).expect("Failed to parse genesis round");

    let context = Arc::new(Context::new_for_test(num_authors as usize).0);
    let mut dag_builder = DagBuilder::new(context);

    // Parse subsequent rounds
    // remove whitespace from the input
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let mut input = cleaned.as_str();
    loop {
        match parse_round(input, &dag_builder) {
            Ok((new_input, (round, connections, transaction_acknowledgments))) => {
                dag_builder.layer_with_connections(connections, transaction_acknowledgments, round);
                input = new_input
            }
            Err(nom::Err::Error(_)) | Err(nom::Err::Failure(_)) => break,
            Err(nom::Err::Incomplete(needed)) => return Err(nom::Err::Incomplete(needed)),
        }
    }
    let (input, _) = tuple((multispace0, char('}')))(input)?;

    Ok(dag_builder)
}

// Parses a round from the input string and returns a tuple containing the round
// number, a vector of connections (authority index and their corresponding
// block references), and a hashmap of transaction acknowledgments for each
// authority index.
fn parse_round<'a>(
    input: &'a str,
    dag_builder: &DagBuilder,
) -> IResult<
    &'a str,
    (
        Round,
        Vec<(AuthorityIndex, Vec<BlockRef>)>,
        HashMap<AuthorityIndex, Vec<BlockRef>>,
    ),
> {
    let (input,(round, connections)) = map(
        tuple((
            map_res(preceded(tag("Round"), alpha1), |s: &str| {
                s.parse::<Round>()
            }),
            char(':'),
            delimited(
                char('{'),
                |i| parse_connections(i, dag_builder),
                char('}')
            ),
            opt(char(',')),
        )),
        |(round,_, connections,_)| (round, connections),
    )(input)?;

    // TODO: extend DAG parser with transaction acknowledgments. For now it's
    //  assumed that transactions are available together with the block headers and
    //  we acknowledge transactions of all ancestors.

    //  If the round is "1", we assume no transactions in Round 0 (genesis) are
    // acknowledged.
    let transactions_acknowledgments: HashMap<AuthorityIndex, Vec<BlockRef>> =
        if round == 1 as Round {
            HashMap::new()
        } else {
            connections.clone().into_iter().collect()
        };
    Ok((input, (round, connections, transactions_acknowledgments)))
}

fn parse_connections<'a>(
    input: &'a str,
    dag_builder: &DagBuilder,
) -> IResult<&'a str, Vec<(AuthorityIndex, Vec<BlockRef>)>> {
    // parse specified connections
    // case 1: all authorities; [*]
    // case 2: specific included authorities; [A0, B0, C0]
    // case 3: specific excluded authorities;  [-A0]
    // case 4: mixed all authorities + specific included/excluded authorities; [*,
    // A0] TODO: case 5: byzantine case of multiple blocks per slot; [*];
    // timestamp=1
    let (input, authors_and_connections) = many0(parse_author_and_connections)(input)?;

    let mut output = Vec::new();
    for (author, connections) in authors_and_connections {
        let mut block_refs = HashSet::new();
        match connections {
            ConnectionSpec::All => {
                // If the connection is "*", we take all last ancestors
                block_refs.extend(dag_builder.last_ancestors.clone());
            }
            ConnectionSpec::Skip(slots) => {
                // If the connection is a skip list, we get the blocks at those slots
                let stored_block_refs = slots
                    .into_iter()
                    .flat_map(|slot| get_blocks(slot, dag_builder))
                    .collect::<HashSet<_>>();
                block_refs.extend(dag_builder.last_ancestors.clone());

                // Retain only those ancestors that are not in the stored block references
                block_refs.retain(|ancestor| !stored_block_refs.contains(ancestor));
            }
            ConnectionSpec::Only(slots) => {
                // If the connection is a list of specific slots, we get the blocks at those
                // slots
                let stored_block_refs = slots
                    .into_iter()
                    .flat_map(|slot| get_blocks(slot, dag_builder))
                    .collect::<HashSet<_>>();
                block_refs.extend(stored_block_refs);
            }
        }
        output.push((author, block_refs.into_iter().collect()));
    }

    Ok((input, output))
}

fn get_blocks(slot: Slot, dag_builder: &DagBuilder) -> Vec<BlockRef> {
    // note: special case for genesis blocks as they are cached separately
    let block_refs = if slot.round == 0 {
        dag_builder
            .genesis_block_refs()
            .into_iter()
            .filter(|block| Slot::from(*block) == slot)
            .collect::<Vec<_>>()
    } else {
        dag_builder
            .get_uncommitted_blocks_at_slot(slot)
            .iter()
            .map(|block| block.reference())
            .collect::<Vec<_>>()
    };
    block_refs
}

// Parses "B1", "C3", "G44" into a Slot
fn alpha_num(input: &str) -> IResult<&str, Slot> {
    map(pair(anychar, digit1), |(anychar, digit): (char, &str)| {
        Slot::new(
            digit.parse::<Round>().unwrap(),
            AuthorityIndex::new_for_test(anychar as u32 - 'A' as u32),
        )
    })(input)
}

// Parses ["B1", "C3", "G44"] into vector of slots
fn only_list(input: &str) -> IResult<&str, Vec<Slot>> {
    separated_list1(char(','), alpha_num)(input)
}

// Parses ["-B4", "-C33", "-G44"] into vector of slots
fn skip_list(input: &str) -> IResult<&str, Vec<Slot>> {
    separated_list1(char(','), preceded(char('-'), alpha_num))(input)
}

// Parses ["*"],["A1", "B2", "C3"], ["-A4", "-B5"] into ConnectionSpec
// - "*"                -> ConnectionSpec::All
// - ["-A4", "-B5"]     -> ConnectionSpec::Skip(Vec<Slot>)
// - ["A1", "B2", "C3"] -> ConnectionSpec::Only(Vec<Slot>)
fn parse_connection_spec(input: &str) -> IResult<&str, ConnectionSpec> {
    alt((
        map(tag("*"), |_| ConnectionSpec::All),
        map(skip_list, ConnectionSpec::Skip),
        map(only_list, ConnectionSpec::Only),
    ))(input)
}
fn parse_author_and_connections(input: &str) -> IResult<&str, (AuthorityIndex, ConnectionSpec)> {
    pair(
        map(terminated(anychar, tag("->")), |author: char| {
            AuthorityIndex::new_for_test(author as u32 - 'A' as u32)
        }),
        delimited(char('['), parse_connection_spec, char(']')),
    )(input)
}

fn parse_genesis(input: &str) -> IResult<&str, u32> {
    let (input, num_authorities) = preceded(
        tuple((
            multispace0,
            tag("Round"),
            space1,
            char('0'),
            space0,
            char(':'),
            space0,
            char('{'),
            space0,
        )),
        |i| parse_authority_count(i),
    )(input)?;
    let (input, _) = tuple((space0, char('}'), opt(char(','))))(input)?;

    Ok((input, num_authorities))
}

fn parse_authority_count(input: &str) -> IResult<&str, u32> {
    let (input, num_str) = digit1(input)?;
    Ok((input, num_str.parse().unwrap()))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::block_header::BlockHeaderAPI;

    #[tokio::test]
    async fn test_dag_parsing() {
        telemetry_subscribers::init_for_testing();
        let dag_str = "DAG { 
            Round 0 : { 4 },
            Round 1 : { * },
            Round 2 : { * },
            Round 3 : {
                A -> [*],
                B -> [*],
                C -> [*],
                D -> [*],
            },
            Round 4 : {
                A -> [A3, B3, C3],
                B -> [A3, B3, C3],
                C -> [A3, B3, C3],
                D -> [*],
            },
            Round 5 : {
                A -> [*],
                B -> [-A4],
                C -> [-A4],
                D -> [-A4],
            },
            Round 6 : {
                A -> [A3, B3, C3, A1, B1],
                B -> [*, A0],
                C -> [-A5],
            }
         }";
        let result = parse_dag(dag_str);
        assert!(result.is_ok());

        let dag_builder = result.unwrap();
        assert_eq!(dag_builder.genesis.len(), 4);
        assert_eq!(dag_builder.block_headers.len(), 23);

        // Check the blocks were correctly parsed in Round 6
        let blocks_a6 = dag_builder
            .get_uncommitted_blocks_at_slot(Slot::new(6, AuthorityIndex::new_for_test(0)));
        assert_eq!(blocks_a6.len(), 1);
        let block_a6 = blocks_a6.first().unwrap();
        assert_eq!(block_a6.round(), 6);
        assert_eq!(block_a6.author(), AuthorityIndex::new_for_test(0));
        assert_eq!(block_a6.ancestors().len(), 5);
        let expected_block_a6_ancestor_slots = [
            Slot::new(3, AuthorityIndex::new_for_test(0)),
            Slot::new(3, AuthorityIndex::new_for_test(1)),
            Slot::new(3, AuthorityIndex::new_for_test(2)),
            Slot::new(1, AuthorityIndex::new_for_test(0)),
            Slot::new(1, AuthorityIndex::new_for_test(1)),
        ];
        for ancestor in block_a6.ancestors() {
            assert!(expected_block_a6_ancestor_slots.contains(&Slot::from(*ancestor)));
        }

        let blocks_b6 = dag_builder
            .get_uncommitted_blocks_at_slot(Slot::new(6, AuthorityIndex::new_for_test(1)));
        assert_eq!(blocks_b6.len(), 1);
        let block_b6 = blocks_b6.first().unwrap();
        assert_eq!(block_b6.round(), 6);
        assert_eq!(block_b6.author(), AuthorityIndex::new_for_test(1));
        assert_eq!(block_b6.ancestors().len(), 5);
        let expected_block_b6_ancestor_slots = [
            Slot::new(5, AuthorityIndex::new_for_test(0)),
            Slot::new(5, AuthorityIndex::new_for_test(1)),
            Slot::new(5, AuthorityIndex::new_for_test(2)),
            Slot::new(5, AuthorityIndex::new_for_test(3)),
            Slot::new(0, AuthorityIndex::new_for_test(0)),
        ];
        for ancestor in block_b6.ancestors() {
            assert!(expected_block_b6_ancestor_slots.contains(&Slot::from(*ancestor)));
        }

        let blocks_c6 = dag_builder
            .get_uncommitted_blocks_at_slot(Slot::new(6, AuthorityIndex::new_for_test(2)));
        assert_eq!(blocks_c6.len(), 1);
        let block_c6 = blocks_c6.first().unwrap();
        assert_eq!(block_c6.round(), 6);
        assert_eq!(block_c6.author(), AuthorityIndex::new_for_test(2));
        assert_eq!(block_c6.ancestors().len(), 3);
        let expected_block_c6_ancestor_slots = [
            Slot::new(5, AuthorityIndex::new_for_test(1)),
            Slot::new(5, AuthorityIndex::new_for_test(2)),
            Slot::new(5, AuthorityIndex::new_for_test(3)),
        ];
        for ancestor in block_c6.ancestors() {
            assert!(expected_block_c6_ancestor_slots.contains(&Slot::from(*ancestor)));
        }
    }

    #[tokio::test]
    async fn test_genesis_round_parsing() {
        let dag_str = "Round 0 : { 4 }";
        let result = parse_genesis(dag_str);
        assert!(result.is_ok());
        let (_, num_authorities) = result.unwrap();

        assert_eq!(num_authorities, 4);
    }

    #[tokio::test]
    async fn test_all_round_parsing() {
        let dag_str = "Round 1 : { * }";
        let context = Arc::new(Context::new_for_test(4).0);
        let dag_builder = DagBuilder::new(context);
        let result = parse_round(dag_str, &dag_builder);
        assert!(result.is_ok());
        let (_, (round, connections, transactions_acknowledgments)) = result.unwrap();

        assert_eq!(round, 1);
        for (i, (authority, references)) in connections.into_iter().enumerate() {
            assert_eq!(authority, AuthorityIndex::new_for_test(i as u32));
            assert_eq!(references, dag_builder.last_ancestors);
            assert!(
                transactions_acknowledgments
                    .get(&authority)
                    .cloned()
                    .unwrap_or_default()
                    .is_empty(),
                "Transactions should not be acknowledged in Round 1"
            );
        }
    }

    #[tokio::test]
    async fn test_specific_round_parsing() {
        let dag_str = "Round 1 : {
            A -> [A0, B0, C0, D0],
            B -> [*, A0],
            C -> [-A0],
        }";
        let context = Arc::new(Context::new_for_test(4).0);
        let dag_builder = DagBuilder::new(context);
        let result = parse_round(dag_str, &dag_builder);
        assert!(result.is_ok());
        let (_, (round, connections, transactions_acknowledgments)) = result.unwrap();

        let skipped_slot = Slot::new_for_test(0, 0); // A0
        let mut expected_references = [
            dag_builder.last_ancestors.clone(),
            dag_builder.last_ancestors.clone(),
            dag_builder
                .last_ancestors
                .into_iter()
                .filter(|ancestor| Slot::from(*ancestor) != skipped_slot)
                .collect(),
        ];

        assert_eq!(round, 1);
        for (i, (authority, mut references)) in connections.into_iter().enumerate() {
            assert_eq!(authority, AuthorityIndex::new_for_test(i as u32));
            references.sort();
            expected_references[i].sort();
            assert_eq!(references, expected_references[i]);

            assert!(
                transactions_acknowledgments.is_empty(),
                "Transactions should not be acknowledged in Round 1"
            );
        }
    }

    // #[tokio::test]
    // async fn test_parse_author_and_connections() {
    //     let expected_authority = str_to_authority_index("A").unwrap();

    //     // case 1: all authorities
    //     let dag_str = "A -> [*]";
    //     let result = parse_author_and_connections(dag_str);
    //     assert!(result.is_ok());
    //     let (_, (actual_author, actual_connections)) = result.unwrap();
    //     assert_eq!(actual_author, expected_authority);
    //     assert_eq!(actual_connections, ["*"]);

    //     // case 2: specific included authorities
    //     let dag_str = "A -> [A0, B0, C0]";
    //     let result = parse_author_and_connections(dag_str);
    //     assert!(result.is_ok());
    //     let (_, (actual_author, actual_connections)) = result.unwrap();
    //     assert_eq!(actual_author, expected_authority);
    //     assert_eq!(actual_connections, ["A0", "B0", "C0"]);

    //     // case 3: specific excluded authorities
    //     let dag_str = "A -> [-A0, -B0]";
    //     let result = parse_author_and_connections(dag_str);
    //     assert!(result.is_ok());
    //     let (_, (actual_author, actual_connections)) = result.unwrap();
    //     assert_eq!(actual_author, expected_authority);
    //     assert_eq!(actual_connections, ["-A0", "-B0"]);

    //     // case 4: mixed all authorities + specific included/excluded authorities
    //     let dag_str = "A -> [*, A0, -B0]";
    //     let result = parse_author_and_connections(dag_str);
    //     assert!(result.is_ok());
    //     let (_, (actual_author, actual_connections)) = result.unwrap();
    //     assert_eq!(actual_author, expected_authority);
    //     assert_eq!(actual_connections, ["*", "A0", "-B0"]);

    //     // TODO: case 5: byzantine case of multiple blocks per slot; [*];
    //     // timestamp=1
    // }
}
