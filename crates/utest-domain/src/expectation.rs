use indexmap::IndexMap;
use serde::{Serialize, Deserialize};

use crate::{
    InterpolatedString,
    ObjectAssertion,
};


/// Expected properties of an HTTP response.
///
/// Omitted status and body assertions impose no requirement. Header entries
/// are the assertions that the executor must evaluate against the response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ResponseExpectation {
    pub status: Option<u16>,
    pub headers: IndexMap<String, HeaderAssertion>,
    pub body: Option<BodyAssertion>
}

/// The expected shape or contents of an HTTP response body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BodyAssertion {
    Json(ObjectAssertion),
    Text(TextAssertion),
    Empty,
}

/// A comparison strategy for a text response body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TextAssertion {
    Exact(InterpolatedString),
    Contains(InterpolatedString),
}

/// A comparison strategy for one HTTP response header.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HeaderAssertion {
    Exists,
    Exact(InterpolatedString),
    Contains(InterpolatedString)
}
