use super::*;

impl PairingHistory {
    /// Arm undo with a snapshot of wallpapers before a pairing change.
    /// Passing empty state or zero duration disables undo.
    pub fn arm_undo(
        &mut self,
        previous_wallpapers: HashMap<String, PathBuf>,
        duration_secs: u64,
        message: impl Into<String>,
    ) {
        if previous_wallpapers.is_empty() || duration_secs == 0 {
            self.undo_state = None;
            return;
        }

        self.undo_state = Some(UndoState {
            previous_wallpapers,
            started_at: Instant::now(),
            duration: Duration::from_secs(duration_secs),
            message: message.into(),
        });
    }

    /// Check if undo is available.
    pub fn can_undo(&self) -> bool {
        if let Some(state) = &self.undo_state {
            state.started_at.elapsed() < state.duration
        } else {
            false
        }
    }

    /// Get undo state for display.
    pub fn undo_state(&self) -> Option<&UndoState> {
        self.undo_state
            .as_ref()
            .filter(|state| state.started_at.elapsed() < state.duration)
    }

    /// Execute undo, returns the wallpapers to restore.
    pub fn do_undo(&mut self) -> Option<HashMap<String, PathBuf>> {
        if self.can_undo() {
            self.undo_state
                .take()
                .map(|state| state.previous_wallpapers)
        } else {
            None
        }
    }

    /// Clear undo state (called when timeout expires).
    /// Returns true when state changed.
    pub fn clear_expired_undo(&mut self) -> bool {
        if let Some(state) = &self.undo_state {
            if state.started_at.elapsed() >= state.duration {
                self.undo_state = None;
                return true;
            }
        }
        false
    }

    /// Get remaining undo time in seconds.
    pub fn undo_remaining_secs(&self) -> Option<u64> {
        self.undo_state().map(|state| {
            state
                .duration
                .saturating_sub(state.started_at.elapsed())
                .as_secs()
        })
    }

    /// Get undo message.
    pub fn undo_message(&self) -> Option<&str> {
        self.undo_state().map(|state| state.message.as_str())
    }
}
