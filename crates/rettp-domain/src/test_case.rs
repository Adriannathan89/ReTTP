use serde::{Deserialize, Serialize};

use crate::{HttpRequestSpec, ResponseExpectation};

/// One executable HTTP unit test.
///
/// The runner resolves its request data, sends the request through the chosen
/// backend adapter, and evaluates the response against `expectation`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestCase {
    pub name: String,
    pub request: HttpRequestSpec,
    pub expectation: ResponseExpectation,
}

impl TestCase {
    /// Creates a named test from its request and expected response.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        request: HttpRequestSpec,
        expectation: ResponseExpectation,
    ) -> Self {
        Self {
            name: name.into(),
            request,
            expectation,
        }
    }
}
