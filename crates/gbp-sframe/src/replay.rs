/// Sliding-window replay-protection for a single `(KID, sender)` pair.
///
/// Maintains a 1024-entry bit-window: `window[i]` is `1` iff counter
/// `base − i` has been received.  `base` is the highest counter seen so far.
///
/// Counters more than 1024 positions behind `base` are unconditionally
/// rejected as too old.  Counters ahead of `base` advance the window.
pub struct ReplayWindow {
    /// Highest counter seen.  Undefined while `!initialized`.
    base: u64,
    /// 1024-bit window stored as 16 × u64 (LSB of word 0 = bit 0 = `base`).
    window: [u64; 16],
    initialized: bool,
}

const WINDOW_SIZE: u64 = 1024;

impl ReplayWindow {
    /// Creates a new, empty replay window.
    pub fn new() -> Self {
        Self {
            base: 0,
            window: [0u64; 16],
            initialized: false,
        }
    }

    /// Checks whether `ctr` is acceptable (not replayed, not too old) and
    /// marks it as seen.
    ///
    /// Returns `Ok(())` on first acceptance; `Err(())` for replays or
    /// overly-old counters.
    pub fn check_and_mark(&mut self, ctr: u64) -> Result<(), ()> {
        if !self.initialized {
            self.base = ctr;
            self.window[0] = 1;
            self.initialized = true;
            return Ok(());
        }

        if ctr > self.base {
            let advance = ctr - self.base;
            // Shift the existing bits "up" (toward higher positions) by
            // `advance` to make room for the new base at bit 0.
            shift_window_up(&mut self.window, advance as usize);
            self.base = ctr;
            // Mark bit 0 = new base = seen.
            self.window[0] |= 1;
            Ok(())
        } else {
            let behind = self.base - ctr;
            if behind >= WINDOW_SIZE {
                return Err(()); // too old
            }
            let word = (behind / 64) as usize;
            let bit = behind % 64;
            if self.window[word] & (1u64 << bit) != 0 {
                return Err(()); // replay
            }
            self.window[word] |= 1u64 << bit;
            Ok(())
        }
    }

    /// Resets the window (call on epoch change).
    pub fn reset(&mut self) {
        self.initialized = false;
        self.window = [0u64; 16];
    }
}

impl Default for ReplayWindow {
    fn default() -> Self {
        Self::new()
    }
}

/// Shifts the 1024-bit packed array "up" by `by` bit positions in place.
///
/// "Up" means: old bit at position `n` moves to position `n + by`.
/// Positions `0..by` are cleared (they represent new, unseen counters
/// between old `base` and the new `base`).
///
/// Iterates from high words to low so the in-place update is safe even when
/// the word shift is zero.
fn shift_window_up(window: &mut [u64; 16], by: usize) {
    if by >= WINDOW_SIZE as usize {
        *window = [0; 16];
        return;
    }
    let word_shift = by / 64;
    let bit_shift = by % 64;

    // Process from the highest word down so we read unmodified source values.
    for i in (0..16).rev() {
        if i < word_shift {
            window[i] = 0;
            continue;
        }
        let src = i - word_shift;
        if bit_shift == 0 {
            window[i] = window[src];
        } else {
            let lo = window[src];
            // `src − 1` supplies the bits that spill into the lower end of
            // the new word when the shift crosses a 64-bit boundary.
            let hi = if src > 0 { window[src - 1] } else { 0 };
            // Old bits in lo move UP (to higher positions) within new word i.
            // Old bits in hi move UP too but overflow into lo-end of new word.
            window[i] = (lo << bit_shift) | (hi >> (64 - bit_shift));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_window_accepts_first() {
        let mut w = ReplayWindow::new();
        assert!(w.check_and_mark(0).is_ok());
    }

    #[test]
    fn replay_is_rejected() {
        let mut w = ReplayWindow::new();
        w.check_and_mark(10).unwrap();
        assert!(w.check_and_mark(10).is_err());
    }

    #[test]
    fn in_order_sequence() {
        let mut w = ReplayWindow::new();
        for i in 0..100u64 {
            assert!(w.check_and_mark(i).is_ok(), "ctr={i} should be accepted");
        }
    }

    #[test]
    fn out_of_order_within_window() {
        let mut w = ReplayWindow::new();
        // Accept 0, then 1023 (advance window), then try 0 again (replay).
        w.check_and_mark(0).unwrap();
        w.check_and_mark(1023).unwrap();
        assert!(
            w.check_and_mark(0).is_err(),
            "0 should be detected as replay"
        );
        // 500 is inside the window and not yet seen.
        assert!(w.check_and_mark(500).is_ok());
    }

    #[test]
    fn too_old_is_rejected() {
        let mut w = ReplayWindow::new();
        w.check_and_mark(2000).unwrap();
        assert!(w.check_and_mark(0).is_err(), "0 is 2000 behind and too old");
    }

    #[test]
    fn window_advance_across_word_boundary() {
        let mut w = ReplayWindow::new();
        w.check_and_mark(63).unwrap();
        w.check_and_mark(127).unwrap(); // crosses 64-bit word boundary
        assert!(w.check_and_mark(63).is_err());
        assert!(w.check_and_mark(127).is_err());
        assert!(w.check_and_mark(100).is_ok());
    }
}
