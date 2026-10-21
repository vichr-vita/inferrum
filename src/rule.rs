use crate::{Error, Fact, FactSpec, Facts};
use std::{any::Any, fmt};

/// Read-only access to exactly the inputs declared by the current rule.
pub struct Inputs<'a> {
    pub(crate) facts: &'a Facts,
    pub(crate) declared: &'a [FactSpec],
}

impl fmt::Debug for Inputs<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The backing collection also contains facts this rule may not read.
        f.debug_struct("Inputs")
            .field("declared", &self.declared)
            .finish_non_exhaustive()
    }
}

impl Inputs<'_> {
    /// Reads a declared input.
    ///
    /// # Errors
    /// Returns an error for an undeclared, absent, or incorrectly typed fact.
    pub fn get<T: Any>(&self, key: Fact<T>) -> Result<&T, Error> {
        let spec = self
            .declared
            .iter()
            .find(|spec| spec.name == key.name())
            .ok_or(Error::UndeclaredRead(key.name()))?;
        spec.check(&key.spec())?;
        self.facts.require(key)
    }
}

type Evaluator = dyn Fn(&Inputs<'_>) -> Result<Facts, Error> + Send + Sync;

/// A named Rust function with explicit fact dependencies and possible outputs.
/// Return an empty [`Facts`] collection when the rule does not apply.
pub struct Rule {
    pub(crate) id: &'static str,
    pub(crate) description: &'static str,
    pub(crate) source: &'static str,
    pub(crate) inputs: Vec<FactSpec>,
    pub(crate) outputs: Vec<FactSpec>,
    pub(crate) evaluate: Box<Evaluator>,
}

impl fmt::Debug for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rule")
            .field("id", &self.id)
            .field("inputs", &self.inputs)
            .field("outputs", &self.outputs)
            .finish_non_exhaustive()
    }
}

impl Rule {
    /// Declares a rule. [`crate::Engine::new`] validates the complete ruleset.
    pub fn new(
        id: &'static str,
        inputs: impl IntoIterator<Item = FactSpec>,
        outputs: impl IntoIterator<Item = FactSpec>,
        evaluate: impl Fn(&Inputs<'_>) -> Result<Facts, Error> + Send + Sync + 'static,
    ) -> Self {
        Self {
            id,
            description: "",
            source: "",
            inputs: inputs.into_iter().collect(),
            outputs: outputs.into_iter().collect(),
            evaluate: Box::new(evaluate),
        }
    }

    /// Adds the business requirement shown in the rule catalog.
    #[must_use]
    pub fn description(mut self, description: &'static str) -> Self {
        self.description = description;
        self
    }
    /// Attaches optional Rust source for documentation and the HTML inspector.
    /// This text is displayed verbatim and never executed by the viewer.
    #[must_use]
    pub fn source(mut self, source: &'static str) -> Self {
        self.source = source;
        self
    }
    /// Returns the optional source excerpt, or an empty string when none was supplied.
    pub fn source_code(&self) -> &'static str {
        self.source
    }
    /// Returns the stable function or rule identifier.
    pub fn id(&self) -> &'static str {
        self.id
    }
    /// Returns the business requirement.
    pub fn requirement(&self) -> &'static str {
        self.description
    }
    /// Returns the required input declarations.
    pub fn inputs(&self) -> &[FactSpec] {
        &self.inputs
    }
    /// Returns the possible output declarations.
    pub fn outputs(&self) -> &[FactSpec] {
        &self.outputs
    }
}
