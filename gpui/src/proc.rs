//! Child-process helpers.

use std::process::Command;

/// Keeps a console window from flashing up on Windows when this GUI app starts a
/// command-line tool (`curl`). No effect elsewhere.
pub fn quiet(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}
