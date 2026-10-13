//! Optional local reader admission. Does not grant publishing or federation rights.
use crate::db::{Id, Repository};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    pub enabled: bool,
    pub max_new_accounts_per_hour: u32,
    pub max_self_enrolled_accounts: u64,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            enabled: false,
            max_new_accounts_per_hour: 100,
            max_self_enrolled_accounts: 10000,
        }
    }
}
impl Policy {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=1_000_000).contains(&self.max_new_accounts_per_hour),
            "enrollment hourly limit outside1..1000000"
        );
        ensure!(
            (1..=1_000_000_000).contains(&self.max_self_enrolled_accounts),
            "enrollment lifetime limit outside1..1000000000"
        );
        Ok(())
    }
}
#[derive(Debug, thiserror::Error)]
pub enum Denied {
    #[error("self-registration is disabled")]
    Disabled,
    #[error("operator decision prevents self-enrollment")]
    Blocked,
    #[error("enrollment creation limit reached")]
    Limit,
}
#[derive(Debug, Serialize)]
pub struct Admission {
    pub publisher: String,
    pub admitted: bool,
    pub created: bool,
    pub permission_version: i64,
}
impl Repository {
    pub async fn enroll_reader(&self, publisher: Id, policy: &Policy) -> Result<Admission> {
        policy.validate()?;
        if !policy.enabled {
            return Err(Denied::Disabled.into());
        }
        let mut c = self.pool.get().await?;
        let tx = c.transaction().await?;
        tx.batch_execute("SET LOCAL statement_timeout='3s'; SET LOCAL lock_timeout='1s'")
            .await?;
        // Only new enrollment uses this small control-plane lock. Library writes
        // do not touch it. Admission-row locking still serializes operator policy.
        let budget=tx.query_one("SELECT window_started,window_created,total_created,clock_timestamp()>=window_started+interval '1 hour' FROM reader_enrollment_budget WHERE singleton FOR UPDATE",&[]).await?;
        let row=tx.query_opt("SELECT allowed,version,enrollment_blocked FROM account_permissions WHERE publisher=$1 FOR UPDATE",&[&publisher.as_slice()]).await?;
        if let Some(row) = &row {
            if row.get::<_, bool>(0) {
                return Ok(Admission {
                    publisher: hex::encode(publisher),
                    admitted: true,
                    created: false,
                    permission_version: row.get(1),
                });
            }
            if row.get::<_, bool>(2) {
                return Err(Denied::Blocked.into());
            }
        }
        let reset: bool = budget.get(3);
        let hourly = if reset { 0 } else { budget.get::<_, i64>(1) };
        if hourly >= i64::from(policy.max_new_accounts_per_hour)
            || budget.get::<_, i64>(2) >= policy.max_self_enrolled_accounts as i64
        {
            return Err(Denied::Limit.into());
        }
        // ON CONFLICT never overrides an operator row inserted while this key was
        // absent. A foreign peer placeholder can enroll only if not blocked.
        let inserted=tx.query_opt("INSERT INTO account_permissions(publisher,allowed,enrollment_blocked) VALUES($1,TRUE,FALSE) ON CONFLICT(publisher) DO UPDATE SET allowed=TRUE,version=account_permissions.version+1 WHERE NOT account_permissions.allowed AND NOT account_permissions.enrollment_blocked RETURNING version",&[&publisher.as_slice()]).await?;
        let Some(inserted) = inserted else {
            let row = tx
                .query_one(
                    "SELECT allowed,version FROM account_permissions WHERE publisher=$1 FOR UPDATE",
                    &[&publisher.as_slice()],
                )
                .await?;
            if !row.get::<_, bool>(0) {
                return Err(Denied::Blocked.into());
            }
            return Ok(Admission {
                publisher: hex::encode(publisher),
                admitted: true,
                created: false,
                permission_version: row.get(1),
            });
        };
        tx.execute("UPDATE reader_enrollment_budget SET window_started=CASE WHEN $1 THEN clock_timestamp() ELSE window_started END,window_created=$2::BIGINT+1,total_created=total_created+1 WHERE singleton",&[&reset,&hourly]).await?;
        let version = inserted.get(0);
        tx.commit().await?;
        Ok(Admission {
            publisher: hex::encode(publisher),
            admitted: true,
            created: true,
            permission_version: version,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_default_and_explicit_bounded_policy() {
        let policy: Policy = serde_json::from_str("{}").unwrap();
        assert!(!policy.enabled);
        assert!(policy.validate().is_ok());
        assert!(
            Policy {
                max_new_accounts_per_hour: 0,
                ..policy.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            Policy {
                max_self_enrolled_accounts: 0,
                ..policy
            }
            .validate()
            .is_err()
        );
        assert!(serde_json::from_str::<Policy>(r#"{"publisher":true}"#).is_err());
    }
}
