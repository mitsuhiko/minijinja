use crate::compiler::instructions::Instruction;
use crate::error::{Error, ErrorKind};

/// Helper for tracking fuel consumption
pub struct FuelTracker {
    // The initial fuel level.
    initial: u64,
    remaining: isize,
}

impl FuelTracker {
    /// Creates a new fuel tracker.
    ///
    pub fn new(fuel: u64) -> FuelTracker {
        FuelTracker {
            initial: fuel,
            remaining: fuel as isize,
        }
    }

    /// Tracks an instruction.  If it runs out of fuel an error is returned.
    pub fn track(&mut self, instr: &Instruction) -> Result<(), Error> {
        let fuel_to_consume = fuel_for_instruction(instr);
        if fuel_to_consume != 0 {
            self.remaining -= fuel_to_consume;
            if self.remaining <= 0 {
                return Err(Error::from(ErrorKind::OutOfFuel));
            }
        }
        Ok(())
    }

    /// Returns the remaining fuel.
    pub fn remaining(&self) -> u64 {
        self.remaining as _
    }

    /// Returns the consumed fuel.
    pub fn consumed(&self) -> u64 {
        self.initial.saturating_sub(self.remaining())
    }
}

/// How much fuel does an instruction consume?
fn fuel_for_instruction(instruction: &Instruction) -> isize {
    match instruction {
        Instruction::BeginCapture(_)
        | Instruction::PushLoop(_)
        | Instruction::PushDidNotIterate
        | Instruction::PushWith
        | Instruction::PopFrame
        | Instruction::PopLoopFrame
        | Instruction::DupTop
        | Instruction::DiscardTop
        | Instruction::PushAutoEscape
        | Instruction::PopAutoEscape => 0,
        #[cfg(feature = "multi_template")]
        Instruction::ExportLocals => 0,
        #[cfg(feature = "macros")]
        Instruction::LoadBlocks | Instruction::BuildMacro(..) | Instruction::Return => 0,
        _ => 1,
    }
}
