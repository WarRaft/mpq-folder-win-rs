use std::io;
pub fn toggle_logging() -> io::Result<()> {
    mpq_folder_win::log::toggle_logging();
    Ok(())
}
