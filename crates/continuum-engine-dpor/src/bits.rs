//! A fixed-width set of transition labels, as a bitset.
//!
//! Every label set the reducer keeps — a stubborn set, a sleep set, the enabled set
//! of a state, the labels explored from it — ranges over one model's label table, so
//! a width fixed at construction and a word per 64 labels is exact. Iteration is
//! ascending by label index, which is the only order the search ever uses (INV-005:
//! no order here depends on anything but the data).
//!
//! Part of the RFC 0004 baseline reducer (correction 1).

/// A set of label indices below a fixed width.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct LabelSet {
    words: Vec<u64>,
}

impl LabelSet {
    /// The empty set over `width` labels.
    pub(crate) fn empty(width: usize) -> Self {
        Self {
            words: vec![0; width.div_ceil(64)],
        }
    }

    /// The cost of one whole-set operation in work units: a unit per word, plus one.
    pub(crate) fn word_units(&self) -> u64 {
        u64::try_from(self.words.len())
            .unwrap_or(u64::MAX)
            .saturating_add(1)
    }

    /// Add `label`. A label beyond the width is ignored: the width is the model's
    /// label count, and no caller holds a label outside it.
    pub(crate) fn insert(&mut self, label: usize) {
        if let Some(word) = self.words.get_mut(label / 64) {
            *word |= 1_u64 << (label % 64);
        }
    }

    /// Whether `label` is a member.
    pub(crate) fn contains(&self, label: usize) -> bool {
        self.words
            .get(label / 64)
            .is_some_and(|word| word & (1_u64 << (label % 64)) != 0)
    }

    /// `self ∪= other`.
    pub(crate) fn union_with(&mut self, other: &Self) {
        for (mine, theirs) in self.words.iter_mut().zip(other.words.iter()) {
            *mine |= *theirs;
        }
    }

    /// `self ∩= other`.
    pub(crate) fn intersect_with(&mut self, other: &Self) {
        for (mine, theirs) in self.words.iter_mut().zip(other.words.iter()) {
            *mine &= *theirs;
        }
    }

    /// `self \= other`.
    pub(crate) fn subtract(&mut self, other: &Self) {
        for (mine, theirs) in self.words.iter_mut().zip(other.words.iter()) {
            *mine &= !*theirs;
        }
    }

    /// Whether every member of `self` is a member of `other`.
    pub(crate) fn is_subset(&self, other: &Self) -> bool {
        self.words
            .iter()
            .zip(other.words.iter())
            .all(|(mine, theirs)| mine & !theirs == 0)
    }

    /// Whether the set has no member.
    pub(crate) fn is_empty(&self) -> bool {
        self.words.iter().all(|word| *word == 0)
    }

    /// The number of members.
    pub(crate) fn count(&self) -> usize {
        self.words
            .iter()
            .map(|word| usize::try_from(word.count_ones()).unwrap_or(64))
            .fold(0_usize, usize::saturating_add)
    }

    /// The members, ascending.
    pub(crate) fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.words
            .iter()
            .enumerate()
            .flat_map(|(index, word)| Bits {
                base: index.saturating_mul(64),
                word: *word,
            })
    }

    /// The members, ascending, collected.
    pub(crate) fn to_vec(&self) -> Vec<usize> {
        self.iter().collect()
    }
}

/// The set bits of one word, ascending.
struct Bits {
    base: usize,
    word: u64,
}

impl Iterator for Bits {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        if self.word == 0 {
            return None;
        }
        let offset = usize::try_from(self.word.trailing_zeros()).unwrap_or(0);
        // Clear the lowest set bit.
        self.word &= self.word.wrapping_sub(1);
        Some(self.base.saturating_add(offset))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::LabelSet;

    #[test]
    fn set_algebra_matches_a_reference_set() {
        let mut left = LabelSet::empty(130);
        let mut right = LabelSet::empty(130);
        for label in [0, 5, 63, 64, 129] {
            left.insert(label);
        }
        for label in [5, 64, 100] {
            right.insert(label);
        }
        assert_eq!(left.to_vec(), vec![0, 5, 63, 64, 129]);
        assert_eq!(left.count(), 5);
        let mut both = left.clone();
        both.intersect_with(&right);
        assert_eq!(both.to_vec(), vec![5, 64]);
        assert!(both.is_subset(&left) && both.is_subset(&right));
        let mut only = left.clone();
        only.subtract(&right);
        assert_eq!(only.to_vec(), vec![0, 63, 129]);
        let mut all = left;
        all.union_with(&right);
        assert_eq!(all.to_vec(), vec![0, 5, 63, 64, 100, 129]);
        all.insert(1000);
        assert!(!all.contains(1000));
        assert!(LabelSet::empty(0).is_empty());
    }
}
