//! Cuerithm: optional per-execution work accounting; no shared mutable state.
use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetExceeded;
impl fmt::Display for BudgetExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Regular expression work limit exceeded")
    }
}
impl core::error::Error for BudgetExceeded {}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Fuel {
    pub remaining: usize,
    pub exhausted: bool,
}
impl Fuel {
    pub fn new(remaining: usize) -> Self {
        Self {
            remaining,
            exhausted: false,
        }
    }
}
#[inline]
pub(crate) fn spend(fuel: &mut Option<Fuel>) -> bool {
    spend_many(fuel, 1)
}
#[inline]
pub(crate) fn spend_many(fuel: &mut Option<Fuel>, amount: usize) -> bool {
    let Some(fuel) = fuel else {
        return true;
    };
    if amount > fuel.remaining {
        fuel.remaining = 0;
        fuel.exhausted = true;
        false
    } else {
        fuel.remaining -= amount;
        true
    }
}
