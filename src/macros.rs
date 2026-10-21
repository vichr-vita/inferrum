/// Declares typed fact constants with stable names.
///
/// Attributes and visibility apply to each generated constant. Names are explicit
/// so renaming a Rust constant does not change diagnostics or graph exports.
///
/// ```
/// inferrum::declare_facts! {
///     /// Annual salary in integer cents.
///     pub SALARY: u64 = "salary_cents";
///     BONUS: u64 = "bonus_cents";
/// }
/// assert_eq!(SALARY.name(), "salary_cents");
/// ```
#[macro_export]
macro_rules! declare_facts {
    ($( $(#[$attribute:meta])* $visibility:vis $key:ident: $ty:ty = $name:expr; )*) => {
        $(
            $(#[$attribute])*
            $visibility const $key: $crate::Fact<$ty> = $crate::Fact::new($name);
        )*
    };
}

/// Builds a [`crate::Facts`] collection, returning `Result<Facts, Error>`.
///
/// Keys and values are evaluated once in declaration order. Insertion errors,
/// including duplicate or empty names, return an error without a partial collection.
///
/// ```
/// use inferrum::{declare_facts, facts};
/// declare_facts! {
///     SCORE: u8 = "score";
///     ACTIVE: bool = "active";
/// }
/// let inputs = facts! { SCORE => 75, ACTIVE => true }?;
/// assert_eq!(inputs.get(SCORE)?, Some(&75));
/// # Ok::<(), inferrum::Error>(())
/// ```
///
/// Values must have the key's declared type:
///
/// ```compile_fail
/// inferrum::declare_facts! { SCORE: u8 = "score"; }
/// let inputs = inferrum::facts! { SCORE => "seventy-five" };
/// ```
#[macro_export]
macro_rules! facts {
    () => {
        ::core::result::Result::<$crate::Facts, $crate::Error>::Ok($crate::Facts::new())
    };
    ($($key:expr => $value:expr),+ $(,)?) => {{
        (|| -> ::core::result::Result<$crate::Facts, $crate::Error> {
            let mut facts = $crate::Facts::new();
            $(facts.insert($key, $value)?;)+
            ::core::result::Result::Ok(facts)
        })()
    }};
}

/// Declares a rule and binds its typed inputs as shared references.
///
/// The identifier becomes the rule ID. The body returns the output value;
/// `?` and `return Err(...)` propagate evaluation errors. Use `-> ? KEY` for a
/// body returning `Option<T>`: `None` produces no fact. Use `-> [KEY, ...]` for
/// multiple or conditional outputs, with a body returning `Result<Facts, Error>`.
/// Fact keys must be paths, such as `SCORE` or `policy::SCORE`.
///
/// The macro returns a [`crate::Rule`], so you can attach a requirement with
/// [`crate::Rule::description`]. It captures the declaration as source text for
/// the inspector. Captured variables move into the rule's closure.
///
/// ```
/// use inferrum::{declare_facts, facts, rule, ruleset};
/// declare_facts! {
///     SCORE: u8 = "score";
///     BONUS: u64 = "bonus";
/// }
/// let engine = ruleset! {
///     inputs [SCORE];
///     rules [
///         rule! { bonus(score: SCORE) -> ? BONUS {
///             (*score > 50).then_some(100_000)
///         }}.description("Scores above 50 receive a bonus."),
///     ];
/// }?;
/// assert_eq!(engine.run(&facts! { SCORE => 75 }?)?.get(BONUS)?, Some(&100_000));
/// assert!(engine.run(&facts! { SCORE => 50 }?)?.is_empty());
/// # Ok::<(), inferrum::Error>(())
/// ```
///
/// The output key checks the body's type at compile time:
///
/// ```compile_fail
/// inferrum::declare_facts! { BONUS: u64 = "bonus"; }
/// let rule = inferrum::rule! { wrong() -> BONUS { "not an amount" } };
/// ```
#[macro_export]
macro_rules! rule {
    ($name:ident($($binding:ident: $input:path),* $(,)?) -> ? $output:path $body:block) => {
        $crate::rule! { @build $name($($binding: $input),*) [$output] {
            match $body {
                ::core::option::Option::Some(value) => $crate::Facts::one($output, value),
                ::core::option::Option::None => ::core::result::Result::Ok($crate::Facts::new()),
            }
        }; ::core::stringify!($name($($binding: $input),*) -> ? $output $body) }
    };
    ($name:ident($($binding:ident: $input:path),* $(,)?) -> [$($output:path),* $(,)?] $body:block) => {
        $crate::rule! { @build $name($($binding: $input),*) [$($output),*] $body;
            ::core::stringify!($name($($binding: $input),*) -> [$($output),*] $body) }
    };
    ($name:ident($($binding:ident: $input:path),* $(,)?) -> $output:path $body:block) => {
        $crate::rule! { @build $name($($binding: $input),*) [$output] {
            let value = $body;
            $crate::Facts::one($output, value)
        }; ::core::stringify!($name($($binding: $input),*) -> $output $body) }
    };
    (@build $name:ident($($binding:ident: $input:path),*) [$($output:path),*] $body:block; $source:expr) => {
        $crate::Rule::new(
            ::core::stringify!($name),
            [$($input.spec()),*],
            [$($output.spec()),*],
            move |_inputs| {
                $(let $binding = _inputs.get($input)?;)*
                $body
            },
        ).source($source)
    };
}

/// Constructs and validates an [`crate::Engine`] from fact keys and rule expressions.
///
/// Returns `Result<Engine, Error>` with the same validation as [`crate::Engine::new`].
/// Rules can come from [`crate::rule!`] or the lower-level [`crate::Rule`] API.
///
/// ```
/// let empty = inferrum::ruleset! { inputs []; rules []; }?;
/// assert!(empty.run(&inferrum::facts! {}?)?.is_empty());
/// # Ok::<(), inferrum::Error>(())
/// ```
#[macro_export]
macro_rules! ruleset {
    (inputs [$($input:path),* $(,)?]; rules [$($rule:expr),* $(,)?];) => {
        $crate::Engine::new([$($input.spec()),*], ::std::vec![$($rule),*])
    };
}
