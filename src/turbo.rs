//! Turbo encoder of VARA.

#[derive(Debug, PartialEq, Eq)]
pub enum TurboError {
    InterleaverLength { expected: usize, got: usize },
    NotAPermutation,
    NotABit { index: usize },
    UnsupportedRate((u8, u8)),
}

/// One step of the recursive systematic encoder: (next state, parity bit).
pub fn rsc_step(state: u8, input: u8) -> (u8, u8) {
    let _ = (state, input);
    todo!()
}

/// Input bit that drives the feedback to zero, used to terminate the trellis.
pub fn termination_input(state: u8) -> u8 {
    let _ = state;
    todo!()
}

pub fn puncture_period(rate: (u8, u8)) -> Option<usize> {
    let _ = rate;
    todo!()
}

pub fn coded_len(k: usize, rate: (u8, u8)) -> Option<usize> {
    let _ = (k, rate);
    todo!()
}

pub fn encode(info: &[u8], interleaver: &[u16], rate: (u8, u8)) -> Result<Vec<u8>, TurboError> {
    let _ = (info, interleaver, rate);
    todo!()
}
