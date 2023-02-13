use crate::compiler::instructions::Instruction;
use crate::error::{Error, ErrorKind};

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;

/// Helper for tracking fuel consumption
pub struct FuelTracker {
    // This should be an AtomicI64 but sadly 32bit targets do not necessarily have
    // AtomicI64 available.
    remaining: AtomicIsize,
}

impl FuelTracker {
    /// Creates a new fuel tracker.
    ///
    /// The fuel tracker is always wrapped in an `Arc` so that it can be
    /// shared across nested invocations of the template evaluation.
    pub fn new(fuel: u64) -> Arc<FuelTracker> {
        Arc::new(FuelTracker {
            remaining: AtomicIsize::new(fuel as isize),
        })
    }

    /// Tracks an instruction.  If it runs out of fuel an error is returned.
    pub fn track(&self, instr: &Instruction) -> Result<(), Error> {
        let fuel_to_consume = fuel_for_instruction(instr);
        if fuel_to_consume != 0 {
            let old_fuel = self.remaining.fetch_sub(fuel_to_consume, Ordering::Relaxed);
            if old_fuel - fuel_to_consume <= 0 {
                return Err(Error::from(ErrorKind::OutOfFuel));
            }
        }
        Ok(())
    }
}

/// How much fuel does an instruction consume?
fn fuel_for_instruction(instruction: &Instruction) -> isize {
    match instruction {
        Instruction::BeginCapture(_)
        | Instruction::LoadBlocks
        | Instruction::RenderParent
        | Instruction::BuildMacro(..)
        | Instruction::ExportLocals
        | Instruction::PushLoop(_)
        | Instruction::PushDidNotIterate
        | Instruction::PushWith
        | Instruction::PopFrame
        | Instruction::DupTop
        | Instruction::DiscardTop
        | Instruction::PushAutoEscape
        | Instruction::PopAutoEscape
        | Instruction::Return => 0,
        _ => 1,
    }
}
