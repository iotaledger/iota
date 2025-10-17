use move_core_types::u256::U256;
use move_ir_types::location::Spanned;
use move_symbol_pool::Symbol;

use crate::{
    diag,
    diagnostics::DiagnosticReporter,
    expansion::ast::{Attribute_, AttributeName_, AttributeValue},
    shared::{
        known_attributes::{AuthenticatorAttribute, KnownAttribute},
        unique_map::UniqueMap,
    },
};

pub fn parse_authenticator_version(
    reporter: &DiagnosticReporter,
    attributes: &UniqueMap<Spanned<KnownAttribute>, Spanned<Attribute_>>,
) -> Option<u8> {
    let Some(attribute) = attributes.get_(&AuthenticatorAttribute.into()) else {
        return None;
    };

    let sp!(authenticator_loc, value) = attribute;
    match value {
        Attribute_::Name(_) => Some(1),
        Attribute_::Assigned(_, attribute_value) => {
            authenticator_version_to_u8(reporter, &attribute_value)
        }
        Attribute_::Parameterized(_, inner_attributes) => {
            let Some(sp!(_, version_value)) = inner_attributes.get_(&AttributeName_::Unknown(
                Symbol::from(AuthenticatorAttribute::VERSION),
            )) else {
                reporter.add_diag(diag!(
                Attributes::InvalidValue,
                (
                    *authenticator_loc,
                    "Missing `version` for authenticator attribute. Expected format: #[authenticator(version = ...)]".to_string()
                )
            ));
                return None;
            };

            match version_value {
                Attribute_::Name(_) | Attribute_::Parameterized(_, _) => None,
                Attribute_::Assigned(_, attribute_value) => {
                    authenticator_version_to_u8(reporter, &attribute_value)
                }
            }
        }
    }
}

fn authenticator_version_to_u8(
    reporter: &DiagnosticReporter,
    attribute_value: &AttributeValue,
) -> Option<u8> {
    use crate::expansion::ast::{AttributeValue_ as EAV, Value_ as EV};

    match attribute_value {
        sp!(_, EAV::Value(sp!(_, EV::U8(value)))) => Some(*value),
        sp!(_, EAV::Value(sp!(_, EV::InferredNum(value)))) if *value <= U256::from(u8::MAX) => {
            Some(value.down_cast_lossy())
        }
        // As a catch all, we reject all other supported attribute value types.
        sp!(_, _) => {
            reporter.add_diag(diag!(
                Attributes::InvalidValue,
                (
                    attribute_value.loc,
                    "Only unannotated or u8 literal `version` values are supported.".to_string()
                ),
            ));
            None
        }
    }
}
