// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{char, sync::Arc};

use nom::{
    IResult,
    branch::alt,
    bytes::complete::tag,
    character::complete::{anychar, char, digit1},
    combinator::{map, map_res, opt},
    multi::{many0, separated_list0},
    sequence::{delimited, pair, preceded, separated_pair, terminated, tuple},
};
use starfish_config::AuthorityIndex;

use crate::{
    block_header::{Round, Slot},
    context::Context,
    test_dag_builder::{AncestorConnectionSpec, AncestorSelection, DagBuilder},
};

/// Parses a non-empty sequence of decimal digits into a `u32` integer.
///
/// # Format
/// - One or more ASCII digits (`0-9`)
///
/// # Returns
/// The parsed `u32` value on success.
fn parse_u32(input: &str) -> IResult<&str, u32> {
    map_res(digit1, |num_str: &str| num_str.parse::<u32>())(input)
}
/// Parses the genesis round definition with the number of authorities.
///
/// # Format
/// - The literal string `"Round0:{"`
/// - Followed by a non-empty sequence of digits representing the number of
///   authorities
/// - Followed by a closing brace `}`
/// - Optionally followed by a comma `,`
///
/// # Returns
/// The parsed `u32` number representing the number of authorities.
fn parse_genesis(input: &str) -> IResult<&str, u32> {
    delimited(
        tag("Round0:{"),
        parse_u32,
        tuple((char('}'), opt(char(',')))),
    )(input)
}
/// Parses a single uppercase ASCII character representing an authority index.
///
/// # Format
/// - A single ASCII uppercase letter (`'A'` to `'Z'`)
///
/// # Returns
/// - `AuthorityIndex` constructed by mapping `'A'` → 0, `'B'` → 1, etc.
fn parse_authority_index(input: &str) -> IResult<&str, AuthorityIndex> {
    map_res(anychar, |c: char| {
        if c.is_ascii_uppercase() {
            Ok((c as u8 - b'A').into())
        } else {
            Err(nom::Err::Error(()))
        }
    })(input)
}
/// Parses an alphanumeric slot identifier in the format `A1`, `B3`, etc.
///
/// # Format
/// - A single uppercase ASCII letter representing the authority (e.g., `'A'`
///   maps to index 0)
/// - Followed by one or more digits representing the round number
fn parse_slot(input: &str) -> IResult<&str, Slot> {
    map(
        pair(parse_authority_index, parse_u32),
        |(authority, digit)| Slot::new(digit, authority),
    )(input)
}
/// Parses a single ancestor selection syntax.
///
/// # Supported Formats
/// - `"*"` → `AncestorSelection::UseLast`
/// - `"-A1"` → `AncestorSelection::ExcludeFrom(Slot::A1)`
/// - `"B2"` → `AncestorSelection::IncludeFrom(Slot::B2)`
fn parse_ancestor_selection(input: &str) -> IResult<&str, AncestorSelection> {
    alt((
        map(tag("*"), |_| AncestorSelection::UseLast),
        map(
            preceded(char('-'), parse_slot),
            AncestorSelection::ExcludeFrom,
        ),
        map(parse_slot, AncestorSelection::IncludeFrom),
    ))(input)
}
/// Parses a comma-separated list of ancestor selections.
///
/// # Format
/// - a bracketed list of ancestor selections, e.g. `[*, -B2, C3]`
fn parse_ancestor_selections(input: &str) -> IResult<&str, Vec<AncestorSelection>> {
    delimited(
        char('['),
        separated_list0(char(','), parse_ancestor_selection),
        char(']'),
    )(input)
}
/// Parses a pair of ancestor connections or a single one.
///
/// # Supported Formats
/// - a comma(',') separated pair of ancestor selections delimited by '()'
///   brackets, e.g.: `([A1,B2],[C3,-D4])`
/// - a single list of ancestor connections, e.g.: ```text [*,-B1]```
///
/// # Returns
/// A tuple of two `Vec<AncestorSelection>` where:
/// - the first vector contains block ancestors
/// - the second vector contains transaction acknowledgements
///
/// # Note:
/// If only one selection is provided, it is duplicated for both
fn parse_pair_of_ancestor_selections(
    input: &str,
) -> IResult<&str, (Vec<AncestorSelection>, Vec<AncestorSelection>)> {
    alt((
        delimited(
            char('('),
            separated_pair(
                parse_ancestor_selections,
                char(','),
                parse_ancestor_selections,
            ),
            char(')'),
        ),
        map(parse_ancestor_selections, |selections| {
            (selections.clone(), selections)
        }),
    ))(input)
}
/// Parses an author identifier followed by their ancestor selections enclosed
/// in brackets.
///
/// # Format
/// - An uppercase ASCII character representing the author (e.g. `'A'`)
/// - Followed by the literal string `"->"`
/// - Followed by
/// - Optionally terminated by a comma `,` (useful for parsing lists)
///
/// # Returns
/// A tuple `(AuthorityIndex, Vec<AncestorSelection>)` where:
/// - `AuthorityIndex` is derived from the author character (`'A'` maps to index
///   0)
/// - `Vec<AncestorSelection>` contains parsed ancestor selections inside the
///   brackets
fn parse_author_and_connections(
    input: &str,
) -> IResult<
    &str,
    (
        (AuthorityIndex, Vec<AncestorSelection>),
        (AuthorityIndex, Vec<AncestorSelection>),
    ),
> {
    map(
        terminated(
            pair(
                map(terminated(anychar, tag("->")), |author: char| {
                    AuthorityIndex::from(author as u8 - b'A')
                }),
                parse_pair_of_ancestor_selections,
            ),
            opt(char(',')),
        ),
        |(authority, (ancestors, transactions))| {
            ((authority, ancestors), (authority, transactions))
        },
    )(input)
}
/// Parses an ancestor connection specification from input.
///
/// # Supported Formats
/// - `"*"` indicating a fully connected ancestor graph
///   (`AncestorConnectionSpec::FullyConnected`)
/// - Or a sequence (zero or more) of author-and-connections entries, e.g.:
///   ```text A->[*, -B1],B->[C2], ... ```
///
/// # Returns
/// An `AncestorConnectionSpec` enum value:
/// - `FullyConnected` if the input is a single `"*"` character
/// - `AuthoritySpecific` with a vector of `(AuthorityIndex,
///   Vec<AncestorSelection>)` parsed by `parse_author_and_connections`
fn parse_connections(input: &str) -> IResult<&str, AncestorConnectionSpec> {
    alt((
        map(char('*'), |_| AncestorConnectionSpec::FullyConnected),
        map(
            many0(parse_author_and_connections),
            |author_and_connections| {
                let (ancestors, transactions): (Vec<_>, Vec<_>) =
                    author_and_connections.into_iter().unzip();
                AncestorConnectionSpec::AuthoritySpecific(
                    ancestors,
                    transactions.into_iter().collect(),
                )
            },
        ),
    ))(input)
}
/// Parses a round definition with its ancestor connection specification.
///
/// # Format
/// - The literal prefix `"Round"`
/// - Followed by one or more digits representing the round number (e.g.
///   `"Round0"`, `"Round42"`)
/// - Followed by a colon `:`
/// - Followed by a block of connections enclosed in braces `{ ... }`
/// - Optionally followed by a comma `,`
///
/// Example input:
/// ```text
/// Round3:{A->[*,-B1],B->[C2]},
/// ```
///
/// # Returns
/// A tuple `(Round, AncestorConnectionSpec)` where:
/// - `Round` is the parsed round number as an integer type
/// - `AncestorConnectionSpec` is parsed from the contents inside the braces
fn parse_round(input: &str) -> IResult<&str, (Round, AncestorConnectionSpec)> {
    map(
        tuple((
            map_res(preceded(tag("Round"), digit1), |s: &str| s.parse::<Round>()),
            char(':'),
            delimited(char('{'), |i| parse_connections(i), char('}')),
            opt(char(',')),
        )),
        |(round, _, connections, _)| (round, connections),
    )(input)
}

/// Custom error wrapper to box `nom` errors into a standard `Error` type
#[derive(Debug)]
pub struct ParseDagError(String);

impl std::fmt::Display for ParseDagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Parse DAG Error: {}", self.0)
    }
}

impl std::error::Error for ParseDagError {}

impl<E: std::fmt::Debug> From<nom::Err<E>> for ParseDagError {
    fn from(err: nom::Err<E>) -> Self {
        ParseDagError(format!("{err:?}"))
    }
}
/// DagParser
///
/// Usage:
///
/// ```ignore
/// use super::test_dag_parser::parse_dag;
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
/// let dag_builder = parse_dag(dag_str).expect("Invalid dag"); // parse DAG DSL
/// dag_builder.print(); // print the parsed DAG
/// ```
pub(crate) fn parse_dag(dag_string: &str) -> Result<DagBuilder, ParseDagError> {
    // Parse subsequent rounds
    // remove whitespace from the input
    let cleaned: String = dag_string.chars().filter(|c| !c.is_whitespace()).collect();
    let input = cleaned.as_str();

    let (mut input, num_authors) = preceded(tag("DAG{"), parse_genesis)(input)?;

    let context = Arc::new(Context::new_for_test(num_authors as usize).0);
    let mut dag_builder = DagBuilder::new(context);
    loop {
        match parse_round(input) {
            Ok((new_input, (round, ancestor_connection_spec))) => {
                dag_builder.layer_with_connections(ancestor_connection_spec, round);
                input = new_input
            }
            Err(nom::Err::Error(_)) | Err(nom::Err::Failure(_)) => break,
            Err(e) => return Err(e.into()),
        }
    }
    // Ensure the DAG ends with a closing brace
    let _ = char::<&str, nom::error::Error<&str>>('}')(input)?;
    Ok(dag_builder)
}

#[cfg(test)]
mod tests {
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
                B -> ([*, A0],[B1, C1]),
                C -> ([-A5],[]),
            }
         }";
        let result = parse_dag(dag_str);
        assert!(result.is_ok());

        let dag_builder = result.unwrap();
        assert_eq!(dag_builder.genesis.len(), 4);
        assert_eq!(dag_builder.block_headers.len(), 23);

        // Check the blocks were correctly parsed in Round 6
        let blocks_a6 = dag_builder.get_uncommitted_blocks_at_slot(Slot::new(6, 0));
        assert_eq!(blocks_a6.len(), 1);
        let block_a6 = blocks_a6.first().unwrap();
        assert_eq!(block_a6.round(), 6);
        assert_eq!(block_a6.author(), 0.into());
        assert_eq!(block_a6.ancestors().len(), 5);
        let expected_block_a6_ancestor_slots = [
            Slot::new(3, 0),
            Slot::new(3, 1),
            Slot::new(3, 2),
            Slot::new(1, 0),
            Slot::new(1, 1),
        ];
        for ancestor in block_a6.ancestors() {
            assert!(expected_block_a6_ancestor_slots.contains(&Slot::from(*ancestor)));
        }
        // all ancestors have corresponding acknowledgments
        // A -> [A3, B3, C3, A1, B1]
        for transaction_ack in block_a6.acknowledgments() {
            assert!(expected_block_a6_ancestor_slots.contains(&Slot::from(*transaction_ack)));
        }

        let blocks_b6 = dag_builder.get_uncommitted_blocks_at_slot(Slot::new(6, 1));
        assert_eq!(blocks_b6.len(), 1);
        let block_b6 = blocks_b6.first().unwrap();
        assert_eq!(block_b6.round(), 6);
        assert_eq!(block_b6.author(), 1.into());
        assert_eq!(block_b6.ancestors().len(), 5);
        let expected_block_b6_ancestor_slots = [
            Slot::new(5, 0),
            Slot::new(5, 1),
            Slot::new(5, 2),
            Slot::new(5, 3),
            Slot::new(0, 0),
        ];
        let expected_block_b6_acknowledgments_slots = [Slot::new(1, 1), Slot::new(1, 2)];
        for ancestor in block_b6.ancestors() {
            assert!(expected_block_b6_ancestor_slots.contains(&Slot::from(*ancestor)));
        }
        // B -> ([*, A0],[B1, C1]),
        for transaction_ack in block_b6.acknowledgments() {
            assert!(
                expected_block_b6_acknowledgments_slots.contains(&Slot::from(*transaction_ack))
            );
        }

        let blocks_c6 = dag_builder.get_uncommitted_blocks_at_slot(Slot::new(6, 2));
        assert_eq!(blocks_c6.len(), 1);
        let block_c6 = blocks_c6.first().unwrap();
        assert_eq!(block_c6.round(), 6);
        assert_eq!(block_c6.author(), 2.into());
        assert_eq!(block_c6.ancestors().len(), 3);
        let expected_block_c6_ancestor_slots = [Slot::new(5, 1), Slot::new(5, 2), Slot::new(5, 3)];
        for ancestor in block_c6.ancestors() {
            assert!(expected_block_c6_ancestor_slots.contains(&Slot::from(*ancestor)));
        }
        // C -> ([-A5],[]),
        assert_eq!(block_c6.acknowledgments().len(), 0);
    }

    #[tokio::test]
    async fn test_genesis_round_parsing() {
        let dag_str = "Round0:{4}";
        let result = parse_genesis(dag_str);
        assert!(result.is_ok());
        let (_, num_authorities) = result.unwrap();

        assert_eq!(num_authorities, 4);
    }

    #[tokio::test]
    async fn test_parse_pair_of_ancestor_selections() {
        let dag_str = "([A1,B2],[C3,-D4])";
        let expected_block_ancestors = parse_ancestor_selections("[A1,B2]").unwrap().1;
        let expected_transaction_acknowledgements =
            parse_ancestor_selections("[C3,-D4]").unwrap().1;
        let result = parse_pair_of_ancestor_selections(dag_str);
        assert!(result.is_ok());
        let (_, (block_ancestors, transaction_acknowledgements)) = result.unwrap();

        assert_eq!(block_ancestors, expected_block_ancestors,);
        assert_eq!(
            transaction_acknowledgements,
            expected_transaction_acknowledgements,
        );
    }

    #[tokio::test]
    async fn test_all_round_parsing() {
        let dag_str = "Round1:{*}";
        let result = parse_round(dag_str);
        assert!(result.is_ok());
        let (_, (round, ancestor_connection_spec)) = result.unwrap();

        assert_eq!(round, 1);
        assert_eq!(
            ancestor_connection_spec,
            AncestorConnectionSpec::FullyConnected
        );
    }

    #[tokio::test]
    async fn test_specific_round_parsing() {
        let dag_str = "Round1:{A->[A0,B0,C0,D0],B->[*,A0],C->[-A0],}";
        let expected_ancestors_for_a = parse_ancestor_selections("[A0,B0,C0,D0]").unwrap().1;
        let expected_ancestors_for_b = parse_ancestor_selections("[*,A0]").unwrap().1;
        let expected_ancestors_for_c = parse_ancestor_selections("[-A0]").unwrap().1;
        let expected_vector = vec![
            (0.into(), expected_ancestors_for_a),
            (1.into(), expected_ancestors_for_b),
            (2.into(), expected_ancestors_for_c),
        ];
        let result = parse_round(dag_str);
        assert!(result.is_ok());
        let (_, (round, ancestor_connection_spec)) = result.unwrap();
        assert_eq!(round, 1);
        assert_eq!(
            ancestor_connection_spec,
            AncestorConnectionSpec::AuthoritySpecific(
                expected_vector.clone(),
                expected_vector.into_iter().collect()
            )
        );
    }

    #[tokio::test]
    async fn test_parse_author_and_connections() {
        let expected_authority = 0.into(); // 'A'

        // case 1: all authorities
        let dag_str = "A->[*]";
        let result = parse_author_and_connections(dag_str);
        assert!(result.is_ok());
        let (_, ((actual_author, actual_connections), _)) = result.unwrap();
        assert_eq!(actual_author, expected_authority);
        assert_eq!(actual_connections, vec![AncestorSelection::UseLast]);

        // case 2: specific included authorities
        let dag_str = "A->[A0,B0,C0]";
        let result = parse_author_and_connections(dag_str);
        assert!(result.is_ok());
        let (_, ((actual_author, actual_connections), _)) = result.unwrap();
        assert_eq!(actual_author, expected_authority);
        assert_eq!(
            actual_connections,
            vec![
                AncestorSelection::IncludeFrom(Slot::new(0, 0)), // A0
                AncestorSelection::IncludeFrom(Slot::new(0, 1)), // B0
                AncestorSelection::IncludeFrom(Slot::new(0, 2)), // C0
            ]
        );

        // case 3: specific excluded authorities
        let dag_str = "A->[-A0,-B0]";
        let result = parse_author_and_connections(dag_str);
        assert!(result.is_ok());
        let (_, ((actual_author, actual_connections), _)) = result.unwrap();
        assert_eq!(actual_author, expected_authority);
        assert_eq!(
            actual_connections,
            vec![
                AncestorSelection::ExcludeFrom(Slot::new(0, 0)), // -A0
                AncestorSelection::ExcludeFrom(Slot::new(0, 1)), // -B0
            ]
        );

        // case 4: mixed all authorities + specific included/excluded authorities
        let dag_str = "A->[*,A0,-B0]";
        let result = parse_author_and_connections(dag_str);
        assert!(result.is_ok());
        let (_, ((actual_author, actual_connections), _)) = result.unwrap();
        assert_eq!(actual_author, expected_authority);
        assert_eq!(
            actual_connections,
            vec![
                AncestorSelection::UseLast,                      // *
                AncestorSelection::IncludeFrom(Slot::new(0, 0)), // A0
                AncestorSelection::ExcludeFrom(Slot::new(0, 1)), // -B0
            ]
        );

        // TODO: case 5: byzantine case of multiple blocks per slot; [*];
        // timestamp=1
    }
}
