use std::path::Path;

use crate::error::AppError;

pub fn resolve_mime_type(path: &Path, explicit: Option<&str>) -> Result<String, AppError> {
    if let Some(explicit) = explicit {
        let explicit = explicit.trim();
        if explicit.is_empty() {
            return Err(AppError::invalid_drive_input(
                "`--mime` cannot be empty when provided",
            ));
        }
        return Ok(explicit.to_string());
    }

    Ok(mime_guess::from_path(path)
        .first_raw()
        .unwrap_or("application/octet-stream")
        .to_string())
}
