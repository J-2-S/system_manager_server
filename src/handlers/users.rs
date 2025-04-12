use tokio::{
    io::AsyncWriteExt,
    process::Command,
    task::{self, JoinError},
};
use users::{self, os::unix::GroupExt, Group, User};
use std::{
    error::Error,
    fmt,
    process::{ExitStatus, Stdio},
};
use crate::auth::{self, is_sudo, USER_LOCK};

#[derive(Debug)]
enum ManagementError {
    PermissionDenied,
    IOError(tokio::io::Error),
    ExitError(ExitStatus),
    TaskError(JoinError),
    CommandFailed(String)
}

impl Error for ManagementError {}

impl fmt::Display for ManagementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommandFailed(error) => write!(f,"{}",error),
            Self::IOError(error) => write!(f, "{}", error),
            Self::ExitError(status) => write!(f, "Command failed with status: {}", status),
            Self::PermissionDenied => write!(f, "You are not a sudo user. Please contact your admin if you think is is a mistake"),
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

pub async fn create_user(user: User, username: &str, password: &str) -> Result<(), ManagementError> {
    let lock = USER_LOCK.lock().await; // We need to lock as we need root for
                                                           // this to work

    if auth::is_sudo(user) {
        let user_status = Command::new("sudo")
            .arg("useradd")
            .arg("-m")
            .arg(username) // was mistakenly using `password`
            .status()
            .await?;

        if user_status.success() {
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
                return Err(ManagementError::from(status));
            }
        } else {
            return Err(ManagementError::from(user_status));
        }
    } else {
        return Err(ManagementError::PermissionDenied);
    }
    drop(lock); //If everything runs with no errors then we should wait to drop the lock till we 
                //are done
    Ok(())
}

pub async fn list_users() -> Result<Vec<String>, ManagementError> {
    let users: Vec<String> = task::spawn_blocking(|| {
        unsafe { users::all_users() }
            .map(|user| user.name().to_str().unwrap().to_string()) // Extract usernames as Strings
            .collect()
    })
    .await?;

    Ok(users)
}


pub async fn list_group(group: Group) -> Result<Vec<String>, ManagementError> {
    let members = task::spawn_blocking(move || {
        group.members().iter().map(|name| name.to_str().unwrap().to_string()).collect()
    })
    .await?;

    Ok(members)
}
pub async fn add_user_to_group(
    current_user: User,
    target_user: String,
    target_group: String,
) -> Result<(), ManagementError> {
    if is_sudo(current_user) {
        let group_name = target_group;
        let username  = target_user;

        // Use `usermod -a -G group user` to add user to group
        let status = Command::new("usermod")
                .arg("-a")
                .arg("-G")
                .arg(&group_name)
                .arg(&username)
                .status().await?;

        if status.success() {
            Ok(())
        } else {
            Err(ManagementError::CommandFailed(format!(
                "Failed to add {} to group {}"
                ,username, group_name
            )))
        }
    } else {
        Err(ManagementError::PermissionDenied)
    }
}

pub async fn remove_user_from_group(
    current_user: User,
    target_user: String,
    target_group: String,
) -> Result<(), ManagementError> {
    let lock = USER_LOCK.lock().await;
    if is_sudo(current_user) {
        let group_name = target_group;
        let username = target_user;

        // First get the user's current groups
        let current_groups_output = Command::new("id")
                .arg("-nG")
                .arg(&username)
                .output().await?;

        if !current_groups_output.status.success() {
            return Err(ManagementError::CommandFailed("Failed to get user's groups".to_string()));
        }

        let current_groups = String::from_utf8_lossy(&current_groups_output.stdout);
        let filtered_groups: Vec<&str> = current_groups
            .split_whitespace()
            .filter(|g| *g != group_name)
            .collect();

        let new_group_list = filtered_groups.join(",");

        // Use `usermod -G group1,group2,... user` to overwrite group list
        let status =Command::new("usermod")
                .arg("-G")
                .arg(&new_group_list)
                .arg(&username)
                .status().await?;
    

        if status.success() {
            drop(lock);
            Ok(())
        } else {
            Err(ManagementError::CommandFailed(format!(
                "Failed to remove {} from group {}",
                username, group_name
            )))
        }
    } else {
        Err(ManagementError::PermissionDenied)
    }
}
