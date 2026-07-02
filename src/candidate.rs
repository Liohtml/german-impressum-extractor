//! Internal provenance substrate: a value plus where it came from. Confidence
//! scoring (TP2) will extend this; kept `pub(crate)` so that is not a breaking
//! change.

use std::ops::Range;

use crate::segment::LabelKind;

pub(crate) struct Candidate<T> {
    pub(crate) value: T,
    pub(crate) span: Range<usize>,
    pub(crate) block: usize,
    pub(crate) label: Option<LabelKind>,
}

impl<T> Candidate<T> {
    pub(crate) fn new(
        value: T,
        span: Range<usize>,
        block: usize,
        label: Option<LabelKind>,
    ) -> Self {
        Candidate {
            value,
            span,
            block,
            label,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carries_value_and_provenance() {
        let c = Candidate::new("10115".to_string(), 3..8, 2, Some(LabelKind::Postal));
        assert_eq!(c.value, "10115");
        assert_eq!(c.span, 3..8);
        assert_eq!(c.block, 2);
        assert_eq!(c.label, Some(LabelKind::Postal));
    }
}
