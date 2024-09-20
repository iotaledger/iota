// Copyright 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::{
    base_types::{IotaAddress, ObjectID},
    coin::{Coin, TreasuryCap},
    gas_coin::GasCoin,
    object::Owner,
    TypeTag,
};

use crate::{
    query::{error::QueryError, txn_object::TxnObject},
    types::{MoveType, MoveTypes},
};

/// A query contains flags for various searchable properties of a iota object.
#[derive(Debug, Default)]
pub struct Query {
    full_type: Option<TypeTag>,
    type_name: Option<String>,
    generics: Option<Vec<TypeTag>>,
    owner: Option<OwnerQuery>,
    is_treasury_cap: Option<bool>,
    is_coin: Option<bool>,
    is_gas_coin: Option<bool>,
}

impl Query {
    /// Create an empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Query by the full type.
    pub fn full_type<T: MoveType>(mut self, package_id: ObjectID) -> Self {
        self.full_type = Some(T::type_tag(package_id));
        self
    }

    /// Query by the name of the type (does not include the address or module
    /// qualifiers).
    pub fn type_name(mut self, name: &str) -> Self {
        self.type_name = Some(name.to_owned());
        self
    }

    /// Query by the generics of the type.
    pub fn generics<G: MoveTypes>(mut self, package_id: ObjectID) -> Self {
        self.generics = Some(G::type_tags(package_id));
        self
    }

    /// Query by the [`Owner`] of the object.
    pub fn owner(mut self, owner: OwnerQuery) -> Self {
        self.owner = Some(owner);
        self
    }

    /// Query by the owner, if it is an address.
    pub fn address_owner(self, owner: IotaAddress) -> Self {
        self.owner(OwnerQuery::AddressOwner(owner))
    }

    /// Query by the owner, if it is another object.
    pub fn object_owner(self, owner: ObjectID) -> Self {
        self.owner(OwnerQuery::ObjectOwner(owner))
    }

    /// Query for shared ownership.
    pub fn shared(self) -> Self {
        self.owner(OwnerQuery::Shared)
    }

    /// Query for immutability.
    pub fn immutable(self) -> Self {
        self.owner(OwnerQuery::Immutable)
    }

    /// Query whether the object is a treasury capability.
    pub fn treasury_cap(mut self, is_treasury_cap: bool) -> Self {
        self.is_treasury_cap = Some(is_treasury_cap);
        self
    }

    /// Query whether the object is a [`Coin`].
    pub fn coin(mut self, is_coin: bool) -> Self {
        self.is_coin = Some(is_coin);
        self
    }

    /// Query whether the object is a [`GasCoin`].
    pub fn gas_coin(mut self, is_gas_coin: bool) -> Self {
        self.is_gas_coin = Some(is_gas_coin);
        self
    }

    fn check<T: TxnObject>(&self, t: &T) -> bool {
        let mut res = true;
        if let Some(ty) = &self.full_type {
            res &= matches!(ty, TypeTag::Struct(s) if s.as_ref() == t.ty())
        }
        if let Some(n) = &self.type_name {
            res &= t.name() == n
        }
        if let Some(g) = &self.generics {
            res &= &t.ty().type_params == g
        }
        if let Some(is_treasury) = self.is_treasury_cap {
            res &= TreasuryCap::is_treasury_type(t.ty()) == is_treasury
        }
        if let Some(is_coin) = self.is_coin {
            res &= Coin::is_coin(t.ty()) == is_coin
        }
        if let Some(is_gas_coin) = self.is_gas_coin {
            res &= GasCoin::is_gas_coin(t.ty()) == is_gas_coin
        }
        if let Some(o) = &self.owner {
            res &= t.owner().is_some_and(|t| o.check(t))
        }
        res
    }
}

impl core::fmt::Display for Query {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut filters = Vec::new();
        if let Some(n) = &self.type_name {
            filters.push(format!("type name: {n}"));
        }
        if let Some(n) = &self.generics {
            let types = n
                .iter()
                .map(|t| t.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            filters.push(format!("generics: <{types}>"));
        }
        if let Some(n) = &self.owner {
            filters.push(format!("owner: {n}"));
        }
        if let Some(n) = self.is_treasury_cap {
            filters.push(format!("is treasury capability: {n}"));
        }
        if let Some(n) = self.is_coin {
            filters.push(format!("is coin: {n}"));
        }
        write!(f, "{{ {} }}", filters.join(", "))
    }
}

/// Queries for ownership of an object.
#[derive(Debug)]
pub enum OwnerQuery {
    /// Owned by an address.
    AddressOwner(IotaAddress),
    /// Owned by another object.
    ObjectOwner(ObjectID),
    /// Shared.
    Shared,
    /// Shared, but immutable.
    Immutable,
}

impl OwnerQuery {
    fn check(&self, owner: &Owner) -> bool {
        match (self, owner) {
            (Self::AddressOwner(a), Owner::AddressOwner(b)) => a == b,
            (Self::ObjectOwner(a), Owner::ObjectOwner(b)) => &IotaAddress::from(*a) == b,
            (Self::Shared, Owner::Shared { .. }) => true,
            (Self::Immutable, Owner::Immutable) => true,
            _ => false,
        }
    }
}

impl core::fmt::Display for OwnerQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OwnerQuery::AddressOwner(a) => write!(f, "address {a}"),
            OwnerQuery::ObjectOwner(a) => write!(f, "object {a}"),
            OwnerQuery::Shared => write!(f, "shared"),
            OwnerQuery::Immutable => write!(f, "immutable"),
        }
    }
}

/// A trait that allows a list of objects to be filtered by a query.
pub trait Queryable<T> {
    /// Filter the objects by the given query, returning the matching subset.
    fn filter<'a>(&'a self, query: Query) -> impl Iterator<Item = &'a T> + '_
    where
        T: 'a;

    /// Filter the objects by the given query and return the first matching
    /// result.
    fn first_matching(&self, query: Query) -> Result<&T, QueryError>;
}

impl<T: TxnObject> Queryable<T> for Vec<T> {
    /// Filter the objects by the given query, returning the matching subset.
    fn filter<'a>(&'a self, query: Query) -> impl Iterator<Item = &'a T> + '_
    where
        T: 'a,
    {
        self.iter().filter(move |o| query.check(*o))
    }

    /// Filter the objects by the given query and return the first matching
    /// result.
    fn first_matching(&self, query: Query) -> Result<&T, QueryError> {
        self.iter()
            .find(|o| query.check(*o))
            .ok_or_else(|| QueryError(query))
    }
}
