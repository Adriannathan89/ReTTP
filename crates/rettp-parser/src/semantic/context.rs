//! Configuration supplied to semantic validation.

use std::collections::HashSet;

use rettp_domain::VariableName;

/// Default maximum nesting depth for values and object assertions.
pub const DEFAULT_MAX_SEMANTIC_DEPTH: usize = 128;
/// Absolute upper bound accepted by [`ValidationContext::with_max_depth`].
///
/// Capping this value protects recursive validation and conversion from
/// unbounded stack growth on programmatically constructed syntax trees.
pub const HARD_MAX_SEMANTIC_DEPTH: usize = 256;

/// External state and safety limits used during semantic validation.
///
/// Predefined variables are visible to every block. Captures discovered in a
/// core block or earlier in a pipeline are added by the validator according to
/// the DSL's scope rules; they are not stored back into this context.
#[derive(Debug, Clone)]
pub struct ValidationContext {
    predefined_variables: HashSet<VariableName>,
    max_depth: usize,
}

impl Default for ValidationContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationContext {
    /// Creates an empty context with [`DEFAULT_MAX_SEMANTIC_DEPTH`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            predefined_variables: HashSet::new(),
            max_depth: DEFAULT_MAX_SEMANTIC_DEPTH,
        }
    }

    /// Adds one variable that is defined before suite validation begins.
    ///
    /// Adding the same variable more than once is idempotent.
    #[must_use]
    pub fn with_predefined_variable(mut self, variable: VariableName) -> Self {
        self.predefined_variables.insert(variable);
        self
    }

    /// Adds multiple variables that are defined before suite validation.
    ///
    /// Duplicate entries are collapsed because the context models a set of
    /// visible names.
    #[must_use]
    pub fn with_predefined_variables(
        mut self,
        variables: impl IntoIterator<Item = VariableName>,
    ) -> Self {
        self.predefined_variables.extend(variables);
        self
    }

    /// Sets the maximum permitted value and assertion nesting depth.
    ///
    /// Values above [`HARD_MAX_SEMANTIC_DEPTH`] are clamped to that constant.
    /// A limit of zero permits only the root level.
    #[must_use]
    pub fn with_max_depth(mut self, limit: usize) -> Self {
        self.max_depth = limit.min(HARD_MAX_SEMANTIC_DEPTH);
        self
    }

    /// Returns the effective maximum nesting depth.
    #[must_use]
    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    pub(crate) fn predefined_variables(&self) -> &HashSet<VariableName> {
        &self.predefined_variables
    }
}
