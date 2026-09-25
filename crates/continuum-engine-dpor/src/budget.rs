//! The work meter: every unit is charged before the work it pays for.
//!
//! Work proportional to the model, the obligations, or the search state goes through
//! the metered helpers here — [`Meter::items`], [`Meter::iter`], [`Meter::vec`] —
//! which charge the whole cost before the first item is touched, so a caller cannot
//! iterate or allocate first and charge after. Where the charge and the work are
//! separate statements (an index search, a set collected into a list), the cost comes
//! from [`Meter::search_cost`] or [`Meter::members_cost`], is charged first, and the
//! work site records it with [`Meter::did`]. Every charge in the crate goes through
//! [`Meter::pay`] (the plain charge is private), and in test builds `pay` requires
//! the previous site's credit to be used up and `did` requires its own site's credit
//! to cover it: a charge removed, or moved after its work, fails every test that
//! reaches the site. What this cannot see is a charge that is present but too small;
//! each site's amount is argued where it is computed.
//!
//! Part of the RFC 0004 baseline reducer (correction 1); charge-before-work is the
//! cr-1wuzry discipline for untrusted models and witnesses.

use crate::bits::LabelSet;
use crate::report::Bound;

/// Work units left, and spent.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Meter {
    left: u64,
    spent: u64,
    /// Units prepaid for a work site and not yet used there (test builds only).
    #[cfg(test)]
    credit: u64,
}

impl Meter {
    /// A meter holding `limit` units.
    pub(crate) const fn new(limit: u64) -> Self {
        Self {
            left: limit,
            spent: 0,
            #[cfg(test)]
            credit: 0,
        }
    }

    /// Spend `units`, or refuse with [`Bound::Work`] and spend nothing. Every
    /// caller goes through [`Meter::pay`], which pairs the charge with its site.
    fn charge(&mut self, units: u64) -> Result<(), Bound> {
        let Some(left) = self.left.checked_sub(units) else {
            return Err(Bound::Work);
        };
        self.left = left;
        self.spent = self.spent.saturating_add(units);
        Ok(())
    }

    /// Units spent so far.
    pub(crate) const fn spent(&self) -> u64 {
        self.spent
    }

    /// Charge `units` for the work that follows, as credit the site's own
    /// [`Meter::did`] consumes before the work. Returns the units. Test builds
    /// require the credit to be zero on entry: no site can live on the leftover of
    /// another.
    #[allow(clippy::panic)]
    pub(crate) fn pay(&mut self, units: u64) -> Result<u64, Bound> {
        #[cfg(test)]
        if self.credit != 0 {
            panic!(
                "unpaid work: {} units of credit left by the last site",
                self.credit
            );
        }
        self.charge(units)?;
        #[cfg(test)]
        {
            self.credit = self.credit.saturating_add(units);
        }
        Ok(units)
    }

    /// Charge `count × per` before work on `count` items, and record it.
    pub(crate) fn items(&mut self, count: usize, per: u64) -> Result<(), Bound> {
        let cost = units(count).saturating_mul(per);
        self.pay(cost)?;
        self.did(cost);
        Ok(())
    }

    /// Charge `len × per` for a slice, then hand out its iterator.
    pub(crate) fn iter<'a, T>(
        &mut self,
        slice: &'a [T],
        per: u64,
    ) -> Result<core::slice::Iter<'a, T>, Bound> {
        self.items(slice.len(), per)?;
        Ok(slice.iter())
    }

    /// Charge `capacity × per`, then allocate.
    pub(crate) fn vec<T>(&mut self, capacity: usize, per: u64) -> Result<Vec<T>, Bound> {
        self.items(capacity, per)?;
        Ok(Vec::with_capacity(capacity))
    }

    /// The cost of one ordered search in a collection of `size` entries whose keys
    /// compare in `key` units (a state's arity, a name's bytes).
    pub(crate) fn search_cost(size: usize, key: usize) -> u64 {
        units(bits(size).saturating_add(1)).saturating_mul(units(key).saturating_add(1))
    }

    /// The cost of reading a label set's members: its words, then one unit per
    /// member for each of `copies` copies made of the list.
    pub(crate) fn members_cost(set: &LabelSet, copies: u64) -> u64 {
        set.word_units()
            .saturating_add(units(set.count()).saturating_mul(copies.max(1)))
    }

    /// Record `units` of work as done at a site that prepaid them. Test builds hold
    /// every site to its own prepayment: using credit no [`Meter::prepay`] provided
    /// panics, so a work site whose charge is removed or moved after it fails every
    /// test that reaches it (other sites' charges cannot mask it).
    #[cfg(test)]
    #[allow(clippy::panic)]
    pub(crate) fn did(&mut self, units: u64) {
        match self.credit.checked_sub(units) {
            Some(left) => self.credit = left,
            None => panic!("unpaid work: {units} units done, {} prepaid", self.credit),
        }
    }

    #[cfg(not(test))]
    #[allow(clippy::unused_self)]
    pub(crate) const fn did(&mut self, _units: u64) {}
}

/// `usize` to work units, saturating.
pub(crate) fn units(count: usize) -> u64 {
    u64::try_from(count).unwrap_or(u64::MAX)
}

/// The bit length of `n` (`⌊log₂ n⌋ + 1`, 0 for 0): an upper bound on the
/// comparisons of one ordered search among `n` entries.
pub(crate) fn bits(n: usize) -> usize {
    usize::try_from(usize::BITS.saturating_sub(n.leading_zeros())).unwrap_or(64)
}
