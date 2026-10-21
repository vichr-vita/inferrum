use std::{
    any::{Any, TypeId, type_name},
    collections::BTreeMap,
    fmt,
    marker::PhantomData,
    sync::Arc,
};

use crate::Error;

/// A named, typed fact key. Declare keys as constants and reuse them in rules.
/// Names identify facts within a ruleset; two keys with the same name must have the same type.
pub struct Fact<T> {
    name: &'static str,
    marker: PhantomData<fn() -> T>,
}

impl<T> Copy for Fact<T> {}
impl<T> Clone for Fact<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> fmt::Debug for Fact<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Fact").field(&self.name).finish()
    }
}

impl<T> Fact<T> {
    /// Creates a key. Empty names are rejected when declared or inserted.
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            marker: PhantomData,
        }
    }
    /// Returns the stable name used in diagnostics and graph exports.
    pub const fn name(self) -> &'static str {
        self.name
    }
}

impl<T: 'static> Fact<T> {
    /// Returns the name and Rust type used to declare a rule dependency or output.
    pub fn spec(self) -> FactSpec {
        FactSpec {
            name: self.name,
            type_id: TypeId::of::<T>(),
            type_name: type_name::<T>(),
        }
    }
}

/// Type metadata for a declared fact. Construct it with [`Fact::spec`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FactSpec {
    pub(crate) name: &'static str,
    type_id: TypeId,
    type_name: &'static str,
}

impl FactSpec {
    /// Returns the fact name.
    pub fn name(&self) -> &'static str {
        self.name
    }
    /// Returns the Rust type name for display.
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }
    pub(crate) fn check(&self, actual: &Self) -> Result<(), Error> {
        if self.type_id == actual.type_id {
            Ok(())
        } else {
            Err(Error::TypeMismatch {
                fact: self.name,
                expected: self.type_name,
                actual: actual.type_name,
            })
        }
    }
}

trait Value: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn as_debug(&self) -> &dyn fmt::Debug;
}

impl<T: Any + fmt::Debug + Send + Sync> Value for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_debug(&self) -> &dyn fmt::Debug {
        self
    }
}

#[derive(Clone)]
pub(crate) struct StoredFact {
    pub(crate) spec: FactSpec,
    value: Arc<dyn Value>,
}

/// An append-only collection of typed facts. Cloning shares values through `Arc`.
/// Keep fact values free of interior mutation when you need immutable snapshots.
#[derive(Clone, Default)]
pub struct Facts {
    pub(crate) values: BTreeMap<&'static str, StoredFact>,
}

impl fmt::Debug for Facts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl Facts {
    /// Creates an empty collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a fact without replacing an existing value.
    ///
    /// # Errors
    /// Returns an error for an empty or already present name.
    pub fn insert<T: Any + fmt::Debug + Send + Sync>(
        &mut self,
        key: Fact<T>,
        value: T,
    ) -> Result<(), Error> {
        use std::collections::btree_map::Entry;
        if key.name.trim().is_empty() {
            return Err(Error::EmptyName);
        }
        match self.values.entry(key.name) {
            Entry::Occupied(_) => Err(Error::DuplicateFact(key.name)),
            Entry::Vacant(entry) => {
                entry.insert(StoredFact {
                    spec: key.spec(),
                    value: Arc::new(value),
                });
                Ok(())
            }
        }
    }

    /// Creates a collection containing one fact.
    ///
    /// # Errors
    /// Returns an error if the name is empty.
    pub fn one<T: Any + fmt::Debug + Send + Sync>(key: Fact<T>, value: T) -> Result<Self, Error> {
        let mut facts = Self::new();
        facts.insert(key, value)?;
        Ok(facts)
    }

    /// Reads a value, returning `None` when it is absent.
    ///
    /// # Errors
    /// Returns an error when the name exists with a different Rust type.
    pub fn get<T: Any>(&self, key: Fact<T>) -> Result<Option<&T>, Error> {
        self.values
            .get(key.name)
            .map(|stored| {
                key.spec().check(&stored.spec)?;
                stored
                    .value
                    .as_any()
                    .downcast_ref::<T>()
                    .ok_or(Error::MissingFact(key.name))
            })
            .transpose()
    }

    /// Reads a required value.
    ///
    /// # Errors
    /// Returns an error when the fact is absent or has a different type.
    pub fn require<T: Any>(&self, key: Fact<T>) -> Result<&T, Error> {
        self.get(key)?.ok_or(Error::MissingFact(key.name))
    }

    /// Returns names and printable values in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &dyn fmt::Debug)> {
        self.values
            .iter()
            .map(|(&name, stored)| (name, stored.value.as_debug()))
    }
    /// Returns the number of facts.
    pub fn len(&self) -> usize {
        self.values.len()
    }
    /// Returns whether no facts are present.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}
