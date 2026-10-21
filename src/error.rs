use std::fmt;

/// A ruleset, fact, or evaluation error.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A fact or rule has an empty name.
    EmptyName,
    /// A fact name was inserted or declared twice.
    DuplicateFact(&'static str),
    /// Two rules have the same identifier.
    DuplicateRule(&'static str),
    /// A fact has more than one source, including initial inputs.
    MultipleProducers(&'static str),
    /// A required fact has no declared source.
    UnknownDependency {
        /// The rule with the missing dependency.
        rule: &'static str,
        /// The fact without a declared source.
        fact: &'static str,
    },
    /// A cycle prevents these rules, and possibly their dependents, from running.
    Cycle {
        /// Rules in or downstream of a dependency cycle.
        blocked_rules: Vec<&'static str>,
    },
    /// A name was used with incompatible Rust types.
    TypeMismatch {
        /// The conflicting fact name.
        fact: &'static str,
        /// The required Rust type.
        expected: &'static str,
        /// The supplied Rust type.
        actual: &'static str,
    },
    /// An initial fact is not among the engine's declared inputs.
    UnexpectedInput(&'static str),
    /// A rule tried to read an undeclared dependency.
    UndeclaredRead(&'static str),
    /// A rule returned a fact outside its declared outputs.
    UndeclaredOutput(&'static str),
    /// A required value is absent.
    MissingFact(&'static str),
    /// A rule callback or its output validation failed.
    RuleFailed {
        /// The callback that failed.
        rule: &'static str,
        /// The underlying application or validation error.
        source: Box<Error>,
    },
    /// A viewer scenario failed during evaluation.
    ViewerCaseFailed {
        /// The scenario identifier.
        case: String,
        /// The underlying evaluation error.
        source: Box<Error>,
    },
    /// An error reported by application logic.
    Custom(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "fact and rule names must not be empty"),
            Self::DuplicateFact(name) => write!(f, "fact {name:?} is declared or inserted twice"),
            Self::DuplicateRule(name) => write!(f, "rule {name:?} is declared twice"),
            Self::MultipleProducers(name) => write!(f, "fact {name:?} has multiple producers"),
            Self::UnknownDependency { rule, fact } => {
                write!(f, "rule {rule:?} depends on fact {fact:?} without a source")
            }
            Self::Cycle { blocked_rules } => write!(
                f,
                "dependency cycle blocks rules: {}",
                blocked_rules.join(", ")
            ),
            Self::TypeMismatch {
                fact,
                expected,
                actual,
            } => write!(f, "fact {fact:?} expects {expected}, received {actual}"),
            Self::UnexpectedInput(name) => {
                write!(f, "fact {name:?} is not a declared initial input")
            }
            Self::UndeclaredRead(name) => write!(f, "rule read undeclared fact {name:?}"),
            Self::UndeclaredOutput(name) => write!(f, "rule returned undeclared fact {name:?}"),
            Self::MissingFact(name) => write!(f, "fact {name:?} is missing"),
            Self::RuleFailed { rule, source } => write!(f, "rule {rule:?} failed: {source}"),
            Self::ViewerCaseFailed { case, source } => write!(f, "viewer case {case:?}: {source}"),
            Self::Custom(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RuleFailed { source, .. } | Self::ViewerCaseFailed { source, .. } => {
                Some(source.as_ref())
            }
            _ => None,
        }
    }
}
