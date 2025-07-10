use std::{
    error::Error,
    fmt,
    process::{ExitStatus, Stdio},
};

use tokio::{
    io::AsyncWriteExt,
    process::Command,
    task::{self, JoinError},
};
use users::os::unix::GroupExt;

#[derive(Debug)]
pub enum ManagementError {
    IOError(tokio::io::Error),
    ExitError(ExitStatus),
    TaskError(JoinError),
    CommandFailed(String),
}

impl Error for ManagementError {}

impl fmt::Display for ManagementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommandFailed(error) => write!(f, "{}", error),
            Self::IOError(error) => write!(f, "{}", error),
            Self::ExitError(status) => write!(f, "Command failed with status: {}", status),
            Self::TaskError(error) => write!(f, "{}", error),
        }
    }
}

impl From<tokio::io::Error> for ManagementError {
    fn from(value: tokio::io::Error) -> Self {
        ManagementError::IOError(value)
    }
}

impl From<ExitStatus> for ManagementError {
    fn from(value: ExitStatus) -> Self {
        ManagementError::ExitError(value)
    }
}

impl From<JoinError> for ManagementError {
    fn from(value: JoinError) -> Self {
        ManagementError::TaskError(value)
    }
}

/// Creates a user and sets their password
pub async fn create_user(username: &str, password: &str) -> Result<(), ManagementError> {
    let user_status = Command::new("sudo")
        .arg("useradd")
        .arg("-m")
        .arg(username)
        .status()
        .await?;

    if !user_status.success() {
        return Err(user_status.into());
    }

    let mut setting_password = Command::new("sudo")
        .arg("chpasswd")
        .stdin(Stdio::piped())
        .spawn()?;

    if let Some(mut input) = setting_password.stdin.take() {
        input
            .write_all(format!("{}:{}\n", username, password).as_bytes())
            .await?;
    }

    let status = setting_password.wait().await?;
    if !status.success() {
        return Err(status.into());
    }

    Ok(())
}

/// Lists all system users
pub async fn list_users() -> Result<Vec<String>, ManagementError> {
    let users = task::spawn_blocking(|| unsafe {
        users::all_users()
            .filter_map(|u| u.name().to_str().map(|s| s.to_string()))
            .collect()
    })
    .await?;

    Ok(users)
}

/// Lists all users in a group by group name
pub async fn list_group_members(group_name: &str) -> Result<Vec<String>, ManagementError> {
    let group_name = group_name.to_owned();
    let members = task::spawn_blocking(move || {
        users::get_group_by_name(&group_name)
            .map(|g| {
                g.members()
                    .iter()
                    .filter_map(|m| m.to_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    })
    .await?;

    Ok(members)
}

/// Adds a user to a group
pub async fn add_user_to_group(username: &str, group_name: &str) -> Result<(), ManagementError> {
    let status = Command::new("sudo")
        .arg("usermod")
        .arg("-a")
        .arg("-G")
        .arg(group_name)
        .arg(username)
        .status()
        .await?;

    if status.success() {
        Ok(())
    } else {
        Err(ManagementError::CommandFailed(format!(
            "Failed to add {} to group {}",
            username, group_name
        )))
    }
}

/// Removes a user from a group
pub async fn remove_user_from_group(
    username: &str,
    group_name: &str,
) -> Result<(), ManagementError> {
    let output = Command::new("id").arg("-nG").arg(username).output().await?;

    if !output.status.success() {
        return Err(ManagementError::CommandFailed(
            "Failed to get user's groups".to_string(),
        ));
    }

    let groups = String::from_utf8_lossy(&output.stdout);
    let new_groups: Vec<&str> = groups
        .split_whitespace()
        .filter(|&g| g != group_name)
        .collect();

    let group_list = new_groups.join(",");

    let status = Command::new("sudo")
        .arg("usermod")
        .arg("-G")
        .arg(&group_list)
        .arg(username)
        .status()
        .await?;

    if status.success() {
        Ok(())
    } else {
        Err(ManagementError::CommandFailed(format!(
            "Failed to remove {} from group {}",
            username, group_name
        )))
    }
}
pub async fn create_group(group_name: &str) -> Result<(), ManagementError> {
    let status = Command::new("sudo")
        .arg("groupadd")
        .arg(group_name)
        .status()
        .await?;
    if status.success() {
        Ok(())
    } else {
        Err(ManagementError::CommandFailed(format!(
            "Failed to create group {}",
            group_name
        )))
    }
}
pub async fn delete_user(username: &str) -> Result<(), ManagementError> {
    let status = Command::new("sudo")
        .arg("userdel")
        .arg("-r")
        .arg(username)
        .status()
        .await?;

    if status.success() {
        Ok(())
    } else {
        Err(ManagementError::CommandFailed(format!(
            "Failed to delete user {}",
            username
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::process::Command;

    const TEST_USER: &str = "fake_user_xyz";
    const TEST_GROUP: &str = "fake_group_xyz";
    const TEST_PASS: &str = "FakePass123";

    async fn cleanup_user() {
        let _ = Command::new("sudo")
            .arg("userdel")
            .arg("-r")
            .arg(TEST_USER)
            .status()
            .await;
    }

    async fn cleanup_group() {
        let _ = Command::new("sudo")
            .arg("groupdel")
            .arg(TEST_GROUP)
            .status()
            .await;
    }

    #[tokio::test]
    async fn test_full_user_group_lifecycle() {
        // Clean up before test (in case it already exists)
        cleanup_user().await;
        cleanup_group().await;

        // Create group
        create_group(TEST_GROUP)
            .await
            .expect("Failed to create test group");

        // Create user
        create_user(TEST_USER, TEST_PASS)
            .await
            .expect("Failed to create test user");

        // Add user to group
        add_user_to_group(TEST_USER, TEST_GROUP)
            .await
            .expect("Failed to add user to group");

        // Confirm user is in group
        let members = list_group_members(TEST_GROUP)
            .await
            .expect("Failed to list group members");
        assert!(members.contains(&TEST_USER.to_string()));

        // Remove user from group
        remove_user_from_group(TEST_USER, TEST_GROUP)
            .await
            .expect("Failed to remove user from group");

        let members_after = list_group_members(TEST_GROUP)
            .await
            .expect("Failed to list group members after removal");
        assert!(!members_after.contains(&TEST_USER.to_string()));

        // Cleanup
        delete_user(TEST_USER).await.expect("Failed to delete user");

        cleanup_group().await;
    }

    #[tokio::test]
    async fn test_list_users_returns_non_empty() {
        let users = list_users().await.expect("Failed to list users");
        assert!(!users.is_empty());
    }

    #[tokio::test]
    async fn test_list_group_members_does_not_panic() {
        // Should not panic even if group doesn't exist
        let _ = list_group_members("nonexistent_group").await;
    }
}
