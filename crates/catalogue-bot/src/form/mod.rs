//! Issue form bodies: the form tables, the section split and the answer records.

mod answer;
mod error;
mod fields;
mod kind;
mod markdown;
mod parse;
mod records;

pub use error::FormError;
pub(crate) use error::excerpt;
pub(crate) use fields::label;
pub use fields::{
    AFFILIATED, CORRECTNESS, DEMOTION, FEATURED_APPROVAL, Field, KIND, MAINTAINER, NOT_AFFILIATED,
    SUBMIT, SUPERSEDES, Shape, TOPIC,
};
pub use kind::{FormKind, LabelError};
pub use parse::parse_form;
pub use records::{Form, JudgmentAnswers, JudgmentForm, SubmitForm};
