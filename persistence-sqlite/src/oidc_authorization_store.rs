use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use rumahl_core::{InstallationId, OidcScope, SessionId, UnixTimestamp, UserId};
use rumahl_oidc_provider::{
    ApprovedAuthorization, AuthorizationCode, AuthorizationTransaction, OidcAuthorizationStore,
    OidcClientRecord, PairwiseSubject, PkceChallenge, UserConsent,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::oidc_schema;

pub struct SqliteOidcAuthorizationStore {
    connection: Mutex<Connection>,
}

#[derive(Debug)]
pub enum SqliteOidcAuthorizationStoreError {
    Database(rusqlite::Error),
    InactiveClient,
    InvalidStoredState,
    LockPoisoned,
}

impl fmt::Display for SqliteOidcAuthorizationStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OIDC authorization storage failed")
    }
}

impl Error for SqliteOidcAuthorizationStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            _ => None,
        }
    }
}

impl SqliteOidcAuthorizationStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqliteOidcAuthorizationStoreError> {
        Self::from_connection(
            Connection::open(path).map_err(SqliteOidcAuthorizationStoreError::Database)?,
        )
    }

    pub fn open_in_memory() -> Result<Self, SqliteOidcAuthorizationStoreError> {
        Self::from_connection(
            Connection::open_in_memory().map_err(SqliteOidcAuthorizationStoreError::Database)?,
        )
    }

    fn from_connection(connection: Connection) -> Result<Self, SqliteOidcAuthorizationStoreError> {
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(SqliteOidcAuthorizationStoreError::Database)?;
        oidc_schema::initialize(&connection)
            .map_err(SqliteOidcAuthorizationStoreError::Database)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }
}

impl OidcAuthorizationStore for SqliteOidcAuthorizationStore {
    type Error = SqliteOidcAuthorizationStoreError;

    fn insert_transaction(
        &self,
        transaction: &AuthorizationTransaction,
    ) -> Result<(), Self::Error> {
        let scopes = encode_scopes(transaction.scopes());
        let connection = self
            .connection
            .lock()
            .map_err(|_| Self::Error::LockPoisoned)?;
        let inserted = connection
            .execute(
                "INSERT INTO oidc_authorization_transaction
             (id, client_id, installation_id, user_id, session_id, redirect_uri, scopes,
              state, nonce, pkce_challenge, expires_at)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11
             WHERE EXISTS (SELECT 1 FROM oidc_client WHERE client_id = ?2 AND revoked_at IS NULL)",
                params![
                    transaction.id(),
                    transaction.client_id().as_str(),
                    transaction.installation_id().to_string(),
                    transaction.user_id().to_string(),
                    transaction.session_id().to_string(),
                    transaction.redirect_uri().as_str(),
                    scopes,
                    transaction.state(),
                    transaction.nonce(),
                    transaction.challenge().as_str(),
                    timestamp(transaction.expires_at())?
                ],
            )
            .map_err(Self::Error::Database)?;
        if inserted != 1 {
            return Err(Self::Error::InactiveClient);
        }
        Ok(())
    }

    fn approve_transaction(
        &self,
        transaction_id: &str,
        client: &OidcClientRecord,
        user_id: UserId,
        session_id: SessionId,
        consent: &UserConsent,
        now: UnixTimestamp,
    ) -> Result<Option<ApprovedAuthorization>, Self::Error> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| Self::Error::LockPoisoned)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(Self::Error::Database)?;
        if !active_client(&transaction, client)? {
            return Ok(None);
        }
        let stored = transaction
            .query_row(
                "SELECT client_id, installation_id, user_id, session_id, redirect_uri, scopes,
                    state, nonce, pkce_challenge, expires_at
             FROM oidc_authorization_transaction
             WHERE id = ?1 AND consumed_at IS NULL AND revoked_at IS NULL",
                [transaction_id],
                |row| {
                    Ok(StoredTransaction {
                        client_id: row.get(0)?,
                        installation_id: row.get(1)?,
                        user_id: row.get(2)?,
                        session_id: row.get(3)?,
                        redirect_uri: row.get(4)?,
                        scopes: row.get(5)?,
                        state: row.get(6)?,
                        nonce: row.get(7)?,
                        challenge: row.get(8)?,
                        expires_at: row.get(9)?,
                    })
                },
            )
            .optional()
            .map_err(Self::Error::Database)?;
        let Some(stored) = stored else {
            return Ok(None);
        };
        if stored.client_id != client.client_id().as_str()
            || stored.installation_id != client.installation_id().to_string()
            || stored.redirect_uri != client.redirect_uri().as_str()
        {
            return Ok(None);
        }
        let stored_user =
            UserId::parse(&stored.user_id).map_err(|_| Self::Error::InvalidStoredState)?;
        let stored_session =
            SessionId::parse(&stored.session_id).map_err(|_| Self::Error::InvalidStoredState)?;
        let expires_at = read_timestamp(stored.expires_at)?;
        let transaction_model = AuthorizationTransaction::restore(
            transaction_id.to_owned(),
            client,
            stored_user,
            stored_session,
            decode_scopes(&stored.scopes)?,
            stored.state,
            stored.nonce,
            PkceChallenge::parse(&stored.challenge).map_err(|_| Self::Error::InvalidStoredState)?,
            expires_at,
        )
        .map_err(|_| Self::Error::InvalidStoredState)?;
        let Ok((code, raw)) = transaction_model.approve(client, consent, user_id, session_id, now)
        else {
            return Ok(None);
        };
        let existing_subject: Option<String> = transaction.query_row(
            "SELECT subject FROM oidc_pairwise_subject WHERE user_id = ?1 AND installation_id = ?2",
            params![user_id.to_string(), client.installation_id().to_string()],
            |row| row.get(0),
        ).optional().map_err(Self::Error::Database)?;
        let subject = match existing_subject {
            Some(value) => PairwiseSubject::restore(user_id, *client.installation_id(), value)
                .map_err(|_| Self::Error::InvalidStoredState)?,
            None => {
                let subject = PairwiseSubject::generate(user_id, *client.installation_id())
                    .map_err(|_| Self::Error::InvalidStoredState)?;
                transaction.execute(
                    "INSERT INTO oidc_pairwise_subject (user_id, installation_id, subject) VALUES (?1, ?2, ?3)",
                    params![user_id.to_string(), client.installation_id().to_string(), subject.value()],
                ).map_err(Self::Error::Database)?;
                subject
            }
        };
        transaction.execute(
            "INSERT INTO oidc_consent (user_id, client_id, installation_id, scopes, explicit_offline_access, granted_at, revoked_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)
             ON CONFLICT(user_id, client_id) DO UPDATE SET scopes = excluded.scopes,
                 explicit_offline_access = excluded.explicit_offline_access,
                 granted_at = excluded.granted_at, revoked_at = NULL",
            params![user_id.to_string(), client.client_id().as_str(), client.installation_id().to_string(),
                encode_scopes(consent.scopes()), i64::from(consent.explicit_offline_access()), timestamp(now)?],
        ).map_err(Self::Error::Database)?;
        transaction
            .execute(
                "INSERT INTO oidc_authorization_code
             (digest, client_id, installation_id, user_id, session_id, redirect_uri,
              scopes, nonce, pkce_challenge, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    &code.digest()[..],
                    client.client_id().as_str(),
                    client.installation_id().to_string(),
                    user_id.to_string(),
                    session_id.to_string(),
                    code.redirect_uri().as_str(),
                    encode_scopes(code.scopes()),
                    code.nonce(),
                    code.challenge().as_str(),
                    timestamp(code.expires_at())?
                ],
            )
            .map_err(Self::Error::Database)?;
        transaction
            .execute(
                "UPDATE oidc_authorization_transaction SET consumed_at = ?2 WHERE id = ?1",
                params![transaction_id, timestamp(now)?],
            )
            .map_err(Self::Error::Database)?;
        transaction.commit().map_err(Self::Error::Database)?;
        Ok(Some(ApprovedAuthorization::new(
            raw,
            transaction_model.state().to_owned(),
            subject,
            client.redirect_uri().clone(),
        )))
    }

    fn consume_code(
        &self,
        raw_code: &str,
        client: &OidcClientRecord,
        redirect_uri: &str,
        verifier: &str,
        now: UnixTimestamp,
    ) -> Result<Option<AuthorizationCode>, Self::Error> {
        if raw_code.len() != 43 {
            return Ok(None);
        }
        let digest = AuthorizationCode::digest_raw(raw_code);
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| Self::Error::LockPoisoned)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(Self::Error::Database)?;
        if !active_client(&transaction, client)? {
            return Ok(None);
        }
        let stored = transaction
            .query_row(
                "SELECT client_id, installation_id, user_id, session_id, redirect_uri, scopes,
                    nonce, pkce_challenge, expires_at FROM oidc_authorization_code
             WHERE digest = ?1 AND consumed_at IS NULL AND revoked_at IS NULL",
                [&digest[..]],
                |row| {
                    Ok(StoredCode {
                        client_id: row.get(0)?,
                        installation_id: row.get(1)?,
                        user_id: row.get(2)?,
                        session_id: row.get(3)?,
                        redirect_uri: row.get(4)?,
                        scopes: row.get(5)?,
                        nonce: row.get(6)?,
                        challenge: row.get(7)?,
                        expires_at: row.get(8)?,
                    })
                },
            )
            .optional()
            .map_err(Self::Error::Database)?;
        let Some(stored) = stored else {
            return Ok(None);
        };
        if stored.client_id != client.client_id().as_str()
            || stored.installation_id != client.installation_id().to_string()
            || stored.redirect_uri != client.redirect_uri().as_str()
        {
            return Ok(None);
        }
        let code = AuthorizationCode::restore(
            digest,
            client,
            UserId::parse(&stored.user_id).map_err(|_| Self::Error::InvalidStoredState)?,
            SessionId::parse(&stored.session_id).map_err(|_| Self::Error::InvalidStoredState)?,
            decode_scopes(&stored.scopes)?,
            stored.nonce,
            PkceChallenge::parse(&stored.challenge).map_err(|_| Self::Error::InvalidStoredState)?,
            read_timestamp(stored.expires_at)?,
        )
        .map_err(|_| Self::Error::InvalidStoredState)?;
        if code
            .verify_exchange(raw_code, client, redirect_uri, verifier, now)
            .is_err()
        {
            return Ok(None);
        }
        let changed = transaction
            .execute(
                "UPDATE oidc_authorization_code SET consumed_at = ?2
             WHERE digest = ?1 AND consumed_at IS NULL AND revoked_at IS NULL",
                params![&digest[..], timestamp(now)?],
            )
            .map_err(Self::Error::Database)?;
        if changed != 1 {
            return Ok(None);
        }
        transaction.commit().map_err(Self::Error::Database)?;
        Ok(Some(code))
    }

    fn find_subject(
        &self,
        user_id: UserId,
        installation_id: InstallationId,
    ) -> Result<Option<PairwiseSubject>, Self::Error> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| Self::Error::LockPoisoned)?;
        let value: Option<String> = connection
            .query_row(
                "SELECT subject FROM oidc_pairwise_subject
                 WHERE user_id = ?1 AND installation_id = ?2",
                params![user_id.to_string(), installation_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(Self::Error::Database)?;
        value
            .map(|value| {
                PairwiseSubject::restore(user_id, installation_id, value)
                    .map_err(|_| Self::Error::InvalidStoredState)
            })
            .transpose()
    }
}

struct StoredTransaction {
    client_id: String,
    installation_id: String,
    user_id: String,
    session_id: String,
    redirect_uri: String,
    scopes: String,
    state: String,
    nonce: String,
    challenge: String,
    expires_at: i64,
}

struct StoredCode {
    client_id: String,
    installation_id: String,
    user_id: String,
    session_id: String,
    redirect_uri: String,
    scopes: String,
    nonce: String,
    challenge: String,
    expires_at: i64,
}

fn active_client(
    transaction: &rusqlite::Transaction<'_>,
    client: &OidcClientRecord,
) -> Result<bool, SqliteOidcAuthorizationStoreError> {
    let found: Option<i64> = transaction.query_row(
        "SELECT 1 FROM oidc_client WHERE client_id = ?1 AND installation_id = ?2 AND revoked_at IS NULL",
        params![client.client_id().as_str(), client.installation_id().to_string()], |row| row.get(0),
    ).optional().map_err(SqliteOidcAuthorizationStoreError::Database)?;
    Ok(found.is_some() && client.is_active())
}

fn encode_scopes(scopes: &[OidcScope]) -> String {
    scopes
        .iter()
        .map(|scope| scope.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

fn decode_scopes(value: &str) -> Result<Vec<OidcScope>, SqliteOidcAuthorizationStoreError> {
    value
        .split(' ')
        .map(|scope| {
            OidcScope::parse(scope).ok_or(SqliteOidcAuthorizationStoreError::InvalidStoredState)
        })
        .collect()
}

fn timestamp(value: UnixTimestamp) -> Result<i64, SqliteOidcAuthorizationStoreError> {
    i64::try_from(value.as_seconds())
        .map_err(|_| SqliteOidcAuthorizationStoreError::InvalidStoredState)
}

fn read_timestamp(value: i64) -> Result<UnixTimestamp, SqliteOidcAuthorizationStoreError> {
    u64::try_from(value)
        .map(UnixTimestamp::from_seconds)
        .map_err(|_| SqliteOidcAuthorizationStoreError::InvalidStoredState)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use rumahl_core::{AppId, InstallationId, OidcClientType};
    use rumahl_oidc_provider::{OidcClientId, OidcClientRepository, OidcRedirectUri};
    use sha2::{Digest, Sha256};

    use crate::SqliteOidcClientRepository;

    const VERIFIER: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~";

    fn client() -> OidcClientRecord {
        OidcClientRecord::restore(
            OidcClientId::generate().unwrap(),
            InstallationId::new(),
            AppId::parse("com.rumahl.cloud").unwrap(),
            "Cloud",
            OidcClientType::Public,
            OidcRedirectUri::parse("https://cloud.rumahl.dev/oidc/callback").unwrap(),
            vec![OidcScope::OpenId, OidcScope::Profile],
            None,
            UnixTimestamp::from_seconds(100),
            None,
        )
        .unwrap()
    }

    #[test]
    fn code_survives_reopen_but_cannot_be_replayed_or_used_by_another_login() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("oidc.sqlite3");
        let client = client();
        SqliteOidcClientRepository::open(&path)
            .unwrap()
            .insert(&client)
            .unwrap();
        let user = UserId::new();
        let session = SessionId::new();
        let challenge =
            PkceChallenge::parse(&URL_SAFE_NO_PAD.encode(Sha256::digest(VERIFIER.as_bytes())))
                .unwrap();
        let transaction = AuthorizationTransaction::start(
            &client,
            client.redirect_uri().as_str(),
            user,
            session,
            vec![OidcScope::OpenId, OidcScope::Profile],
            "state-opaque-123",
            "nonce-opaque-123",
            challenge,
            UnixTimestamp::from_seconds(110),
        )
        .unwrap();
        let id = transaction.id().to_owned();
        SqliteOidcAuthorizationStore::open(&path)
            .unwrap()
            .insert_transaction(&transaction)
            .unwrap();
        let store = SqliteOidcAuthorizationStore::open(&path).unwrap();
        let consent = UserConsent::new(
            user,
            &client,
            vec![OidcScope::OpenId, OidcScope::Profile],
            false,
        )
        .unwrap();
        assert!(
            store
                .approve_transaction(
                    &id,
                    &client,
                    user,
                    SessionId::new(),
                    &consent,
                    UnixTimestamp::from_seconds(111)
                )
                .unwrap()
                .is_none()
        );
        let approved = store
            .approve_transaction(
                &id,
                &client,
                user,
                session,
                &consent,
                UnixTimestamp::from_seconds(111),
            )
            .unwrap()
            .unwrap();
        assert_eq!(approved.state(), "state-opaque-123");
        assert!(
            store
                .approve_transaction(
                    &id,
                    &client,
                    user,
                    session,
                    &consent,
                    UnixTimestamp::from_seconds(112)
                )
                .unwrap()
                .is_none()
        );
        let subject = approved.subject().value().to_owned();
        let raw = approved.code().to_owned();
        drop(store);

        let store = SqliteOidcAuthorizationStore::open(&path).unwrap();
        assert!(
            store
                .consume_code(
                    &raw,
                    &client,
                    client.redirect_uri().as_str(),
                    "wrong",
                    UnixTimestamp::from_seconds(112)
                )
                .unwrap()
                .is_none()
        );
        let code = store
            .consume_code(
                &raw,
                &client,
                client.redirect_uri().as_str(),
                VERIFIER,
                UnixTimestamp::from_seconds(112),
            )
            .unwrap()
            .unwrap();
        assert_eq!(code.user_id(), user);
        assert!(
            store
                .consume_code(
                    &raw,
                    &client,
                    client.redirect_uri().as_str(),
                    VERIFIER,
                    UnixTimestamp::from_seconds(113)
                )
                .unwrap()
                .is_none()
        );
        let saved_subject: String = store.connection.lock().unwrap().query_row(
            "SELECT subject FROM oidc_pairwise_subject WHERE user_id = ?1 AND installation_id = ?2",
            params![user.to_string(), client.installation_id().to_string()], |row| row.get(0),
        ).unwrap();
        assert_eq!(saved_subject, subject);
    }

    #[test]
    fn uninstall_revokes_pending_transactions_and_codes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("oidc.sqlite3");
        let client = client();
        let client_repo = SqliteOidcClientRepository::open(&path).unwrap();
        client_repo.insert(&client).unwrap();
        let user = UserId::new();
        let session = SessionId::new();
        let challenge =
            PkceChallenge::parse(&URL_SAFE_NO_PAD.encode(Sha256::digest(VERIFIER.as_bytes())))
                .unwrap();
        let transaction = AuthorizationTransaction::start(
            &client,
            client.redirect_uri().as_str(),
            user,
            session,
            vec![OidcScope::OpenId],
            "state-opaque-123",
            "nonce-opaque-123",
            challenge,
            UnixTimestamp::from_seconds(110),
        )
        .unwrap();
        let store = SqliteOidcAuthorizationStore::open(&path).unwrap();
        store.insert_transaction(&transaction).unwrap();
        let consent = UserConsent::new(user, &client, vec![OidcScope::OpenId], false).unwrap();
        let approved = store
            .approve_transaction(
                transaction.id(),
                &client,
                user,
                session,
                &consent,
                UnixTimestamp::from_seconds(111),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            client_repo
                .revoke_for_installation(client.installation_id(), UnixTimestamp::from_seconds(112))
                .unwrap(),
            1
        );
        assert!(
            store
                .consume_code(
                    approved.code(),
                    &client,
                    client.redirect_uri().as_str(),
                    VERIFIER,
                    UnixTimestamp::from_seconds(113)
                )
                .unwrap()
                .is_none()
        );
        let live_codes: i64 = store
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM oidc_authorization_code WHERE revoked_at IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(live_codes, 0);
    }

    #[test]
    fn two_local_users_get_distinct_stable_pairwise_subjects() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("oidc.sqlite3");
        let client = client();
        SqliteOidcClientRepository::open(&path)
            .unwrap()
            .insert(&client)
            .unwrap();
        let store = SqliteOidcAuthorizationStore::open(&path).unwrap();
        let mut subjects = Vec::new();
        for user in [UserId::new(), UserId::new(), UserId::new()] {
            let session = SessionId::new();
            let challenge =
                PkceChallenge::parse(&URL_SAFE_NO_PAD.encode(Sha256::digest(VERIFIER.as_bytes())))
                    .unwrap();
            let transaction = AuthorizationTransaction::start(
                &client,
                client.redirect_uri().as_str(),
                user,
                session,
                vec![OidcScope::OpenId],
                "state-opaque-123",
                "nonce-opaque-123",
                challenge,
                UnixTimestamp::from_seconds(110),
            )
            .unwrap();
            store.insert_transaction(&transaction).unwrap();
            let consent = UserConsent::new(user, &client, vec![OidcScope::OpenId], false).unwrap();
            let approved = store
                .approve_transaction(
                    transaction.id(),
                    &client,
                    user,
                    session,
                    &consent,
                    UnixTimestamp::from_seconds(111),
                )
                .unwrap()
                .unwrap();
            subjects.push((user, approved.subject().value().to_owned()));
        }
        assert_ne!(subjects[0].1, subjects[1].1);
        drop(store);
        let store = SqliteOidcAuthorizationStore::open(&path).unwrap();
        for (user, subject) in subjects {
            let saved: String = store.connection.lock().unwrap().query_row(
                "SELECT subject FROM oidc_pairwise_subject WHERE user_id = ?1 AND installation_id = ?2",
                params![user.to_string(), client.installation_id().to_string()], |row| row.get(0),
            ).unwrap();
            assert_eq!(saved, subject);
        }
    }
}
