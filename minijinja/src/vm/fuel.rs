#![cfg_attr(not(feature = "builtins"), allow(dead_code))]
//! Fuel tracking.
//!
//! Fuel is a single budget for all the work a template can make the engine
//! do.  Every instruction costs one fuel, and on top of that the engine
//! charges for the data it produces and the items it visits:
//!
//! * every byte written to the output (including captured output such as
//!   `{% set %}` blocks and macro bodies),
//! * every byte of strings and every slot of lists and maps created by
//!   operators (`~`, `+`, `*`, slicing), literals and the results of filters,
//!   tests, functions and methods,
//! * every byte of string input to a filter,
//! * every item that a built-in filter or operator visits, plus the cost of
//!   comparisons for sorting.
//!
//! Internally fuel is tracked in units of [`UNITS_PER_FUEL`] so that cheap
//! operations such as individual bytes can be charged without rounding.
//!
//! Allocations whose size is known ahead of time are checked before they are
//! performed so that a single huge allocation cannot slip through.
//!
//! Once the fuel runs out, the tracker stays exhausted: every later charge
//! fails.  That way code that cannot propagate an error directly (such as an
//! iterator) can stop early and the engine fails at the next check.
use std::cell::Cell;

use crate::compiler::instructions::Instruction;
use crate::error::{Error, ErrorKind};
use crate::value::{Value, ValueMap, ValueRepr};

/// The internal resolution of fuel.
///
/// One fuel is worth this many units, and one byte costs one unit.  This
/// means that one fuel pays for this many bytes of data.
pub(crate) const UNITS_PER_FUEL: u64 = 64;

/// Cost of one instruction.
pub(crate) const INSTRUCTION_COST: u64 = UNITS_PER_FUEL;

/// Cost of one byte of data produced or processed.
pub(crate) const BYTE_COST: u64 = 1;

/// Cost of visiting one item in a built-in filter or operator.
pub(crate) const ITEM_COST: u64 = 32;

/// Cost of comparing two values while sorting or deduplicating.
pub(crate) const COMPARE_COST: u64 = 64;

/// Cost of one slot in a list (half that of a map entry).  A value is 24
/// bytes, the rest accounts for the growth of the list.
pub(crate) const SLOT_COST: u64 = crate::value::ops::SLOT_COST;

/// Cost of one byte of serialized output (`tojson`, `pprint` and converting
/// containers to strings).  Serializing walks the whole value which is a lot
/// slower than copying bytes.
pub(crate) const SERIALIZED_BYTE_COST: u64 = 16;

/// Cost of one format specifier in the `format` filter.
pub(crate) const FORMAT_SPEC_COST: u64 = 4 * UNITS_PER_FUEL;

/// Helper for tracking fuel consumption.
pub(crate) struct FuelTracker {
    // The initial fuel level.
    initial: u64,
    // The remaining fuel in units.
    remaining: Cell<i64>,
}

impl FuelTracker {
    /// Creates a new fuel tracker.
    pub fn new(fuel: u64) -> FuelTracker {
        FuelTracker {
            initial: fuel,
            remaining: Cell::new(
                i64::try_from(fuel.saturating_mul(UNITS_PER_FUEL)).unwrap_or(i64::MAX),
            ),
        }
    }

    /// Consumes the given number of units.
    ///
    /// Fails if this exhausts the fuel or if the fuel was already exhausted.
    #[inline]
    pub fn consume(&self, units: u64) -> Result<(), Error> {
        let remaining = self
            .remaining
            .get()
            .saturating_sub(i64::try_from(units).unwrap_or(i64::MAX));
        self.remaining.set(remaining);
        if remaining <= 0 {
            Err(Error::from(ErrorKind::OutOfFuel))
        } else {
            Ok(())
        }
    }

    /// Checks that the given number of units could still be consumed.
    ///
    /// This is used before allocating something of a known size.  It does not
    /// consume the fuel as the result is charged once it exists, but if the
    /// fuel would run out, the tracker is exhausted immediately.
    #[inline]
    pub fn ensure(&self, units: u64) -> Result<(), Error> {
        let remaining = self.remaining.get();
        if remaining <= 0 || units >= remaining as u64 {
            self.remaining.set(0);
            Err(Error::from(ErrorKind::OutOfFuel))
        } else {
            Ok(())
        }
    }

    /// Tracks an instruction.  If it runs out of fuel an error is returned.
    #[inline]
    pub fn track(&self, instr: &Instruction) -> Result<(), Error> {
        if consumes_fuel(instr) {
            self.consume(INSTRUCTION_COST)
        } else {
            Ok(())
        }
    }

    /// Returns the remaining fuel in units.
    #[inline]
    pub fn remaining_units(&self) -> u64 {
        self.remaining.get().max(0) as u64
    }

    /// Returns the remaining fuel.
    pub fn remaining(&self) -> u64 {
        self.remaining.get().max(0) as u64 / UNITS_PER_FUEL
    }

    /// Returns the consumed fuel.
    pub fn consumed(&self) -> u64 {
        self.initial.saturating_sub(self.remaining())
    }
}

/// Does an instruction consume fuel?
fn consumes_fuel(instruction: &Instruction) -> bool {
    !matches!(
        instruction,
        Instruction::BeginCapture(_)
            | Instruction::PushLoop(_)
            | Instruction::PushDidNotIterate
            | Instruction::PushWith
            | Instruction::PopFrame
            | Instruction::PopLoopFrame
            | Instruction::DupTop
            | Instruction::DiscardTop
            | Instruction::PushAutoEscape
            | Instruction::PopAutoEscape
    ) && !is_free_feature_instruction(instruction)
}

#[allow(unused_variables)]
fn is_free_feature_instruction(instruction: &Instruction) -> bool {
    #[cfg(feature = "multi_template")]
    if matches!(instruction, Instruction::ExportLocals) {
        return true;
    }
    #[cfg(feature = "multi_template")]
    if matches!(instruction, Instruction::LoadBlocks) {
        return true;
    }
    #[cfg(feature = "macros")]
    if matches!(
        instruction,
        Instruction::BuildMacro(..) | Instruction::Return
    ) {
        return true;
    }
    false
}

/// Returns the cost of `n` items of the given unit cost.
#[inline]
pub(crate) fn cost_of(n: usize, unit: u64) -> u64 {
    (n as u64).saturating_mul(unit)
}

/// Returns the cost of sorting `n` items.
pub(crate) fn sort_cost(n: usize) -> u64 {
    cost_of(n, lookup_cost(n))
}

/// Returns the cost of looking up an item among `n` sorted items.
pub(crate) fn lookup_cost(n: usize) -> u64 {
    let log = usize::BITS - n.leading_zeros();
    COMPARE_COST.saturating_mul(log as u64)
}

/// Returns the size cost of a value.
///
/// This is the cost of the value itself, not of the values it contains: a
/// string costs its length and a list costs a slot per item.  Items were
/// already charged when they were created.  Objects other than the built-in
/// lists and maps are free as the engine cannot know how large they are.
pub(crate) fn value_cost(value: &Value) -> u64 {
    match value.0 {
        ValueRepr::String(ref s, _) => cost_of(s.len(), BYTE_COST),
        ValueRepr::SmallStr(ref s) => cost_of(s.as_str().len(), BYTE_COST),
        ValueRepr::Bytes(ref b) => cost_of(b.len(), BYTE_COST),
        ValueRepr::Object(ref obj) => {
            if let Some(seq) = obj.downcast_ref::<Vec<Value>>() {
                cost_of(seq.len(), SLOT_COST)
            } else if let Some(map) = obj.downcast_ref::<ValueMap>() {
                cost_of(map.len(), 2 * SLOT_COST)
            } else if let Some(tuple) = obj.downcast_ref::<crate::value::Tuple>() {
                cost_of(tuple.len(), SLOT_COST)
            } else {
                0
            }
        }
        _ => 0,
    }
}

/// An iterator adapter that charges fuel for every item.
///
/// When the fuel runs out, it yields a single invalid value that carries the
/// out of fuel error and then ends.
pub(crate) struct FuelIter<'a, I> {
    tracker: Option<&'a FuelTracker>,
    iter: I,
    done: bool,
}

impl<'a, I> FuelIter<'a, I> {
    pub fn new(tracker: Option<&'a FuelTracker>, iter: I) -> Self {
        FuelIter {
            tracker,
            iter,
            done: false,
        }
    }
}

impl<I: Iterator<Item = Value>> Iterator for FuelIter<'_, I> {
    type Item = Value;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        if let Some(tracker) = self.tracker {
            if let Err(err) = tracker.consume(ITEM_COST) {
                self.done = true;
                return Some(Value::from(err));
            }
        }
        self.iter.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.done {
            (0, Some(0))
        } else {
            let (lower, upper) = self.iter.size_hint();
            (lower, upper.map(|x| x.saturating_add(1)))
        }
    }
}
