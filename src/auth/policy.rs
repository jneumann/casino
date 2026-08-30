//! Rules for what counts as an acceptable username and password. Shared by the
//! public signup endpoint and the `create-user` CLI so both agree.

pub const MIN_USERNAME_LEN: usize = 3;
pub const MAX_USERNAME_LEN: usize = 24;
pub const MIN_PASSWORD_LEN: usize = 10;
pub const MAX_PASSWORD_LEN: usize = 128;

/// Checks a username against the signup rules, returning a message suitable for
/// showing to whoever typed it.
pub fn validate_username(username: &str) -> Result<(), String> {
    let length = username.chars().count();

    if !(MIN_USERNAME_LEN..=MAX_USERNAME_LEN).contains(&length) {
        return Err(format!(
            "username must be between {MIN_USERNAME_LEN} and {MAX_USERNAME_LEN} characters long"
        ));
    }

    if !username
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err("username may only contain letters, numbers, underscores, and hyphens".into());
    }

    if !username.starts_with(|ch: char| ch.is_ascii_alphanumeric()) {
        return Err("username must start with a letter or number".into());
    }

    Ok(())
}

/// Checks a password against the signup rules. The upper bound keeps Argon2
/// from being handed an arbitrarily long input.
pub fn validate_password(password: &str) -> Result<(), String> {
    let length = password.chars().count();

    if length < MIN_PASSWORD_LEN {
        return Err(format!(
            "password must be at least {MIN_PASSWORD_LEN} characters long"
        ));
    }

    if length > MAX_PASSWORD_LEN {
        return Err(format!(
            "password must be at most {MAX_PASSWORD_LEN} characters long"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_usernames() {
        assert!(validate_username("high_roller").is_ok());
        assert!(validate_username("ace-of-spades").is_ok());
        assert!(validate_username("7even").is_ok());
    }

    #[test]
    fn rejects_bad_usernames() {
        assert!(validate_username("ab").is_err());
        assert!(validate_username(&"a".repeat(MAX_USERNAME_LEN + 1)).is_err());
        assert!(validate_username("has space").is_err());
        assert!(validate_username("dealer!").is_err());
        assert!(validate_username("_leading").is_err());
    }

    #[test]
    fn enforces_password_length() {
        assert!(validate_password(&"x".repeat(MIN_PASSWORD_LEN)).is_ok());
        assert!(validate_password(&"x".repeat(MIN_PASSWORD_LEN - 1)).is_err());
        assert!(validate_password(&"x".repeat(MAX_PASSWORD_LEN + 1)).is_err());
    }
}
