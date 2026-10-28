use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rumahl_core::{
    AccountState, AccountStateRepository, AppId, InstallationId, OidcClientType, OidcScope,
    UnixTimestamp,
};
use rumahl_oidc_provider::{
    OidcClientId, OidcClientRecord, OidcClientRepository, OidcProtocol, OidcProtocolError,
    OidcRedirectUri, OidcSigningKey,
};
use rumahl_persistence_sqlite::{
    SqliteAccountStateRepository, SqliteOidcAccessTokenStore, SqliteOidcAuthorizationStore,
    SqliteOidcClientRepository,
};
use sha2::{Digest, Sha256};

const VERIFIER: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~";
type Provider = OidcProtocol<
    SqliteOidcClientRepository,
    SqliteOidcAuthorizationStore,
    SqliteOidcAccessTokenStore,
    SqliteAccountStateRepository,
>;

fn provider(path: &std::path::Path) -> Provider {
    OidcProtocol::new(
        "https://rumahl.dev",
        SqliteOidcClientRepository::open(path).unwrap(),
        SqliteOidcAuthorizationStore::open(path).unwrap(),
        SqliteOidcAccessTokenStore::open(path).unwrap(),
        SqliteAccountStateRepository::open(path).unwrap(),
        OidcSigningKey::new("test-key", [7; 32]).unwrap(),
    )
    .unwrap()
}

#[test]
fn two_local_users_recover_codes_and_tokens_without_identity_mixup() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("oidc.sqlite3");
    let accounts = SqliteAccountStateRepository::open(&path).unwrap();
    let mut state = AccountState::new();
    let alice = state.create_account("alice", "Alice").unwrap();
    let bob = state.create_account("bob", "Bob").unwrap();
    let alice_session = state
        .start_session(
            &alice,
            UnixTimestamp::from_seconds(100),
            UnixTimestamp::from_seconds(1000),
        )
        .unwrap();
    let bob_session = state
        .start_session(
            &bob,
            UnixTimestamp::from_seconds(100),
            UnixTimestamp::from_seconds(1000),
        )
        .unwrap();
    accounts.store(&state).unwrap();

    let client = OidcClientRecord::restore(
        OidcClientId::generate().unwrap(),
        InstallationId::new(),
        AppId::parse("com.rumahl.cloud").unwrap(),
        "Nextcloud",
        OidcClientType::Public,
        OidcRedirectUri::parse("https://cloud.rumahl.dev/apps/user_oidc/code").unwrap(),
        vec![OidcScope::OpenId, OidcScope::Profile],
        None,
        UnixTimestamp::from_seconds(100),
        None,
    )
    .unwrap();
    SqliteOidcClientRepository::open(&path)
        .unwrap()
        .insert(&client)
        .unwrap();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(VERIFIER.as_bytes()));
    let redirect = client.redirect_uri().as_str();
    let scopes = vec![OidcScope::OpenId, OidcScope::Profile];

    let mut nonce_bytes = [0_u8; 32];
    getrandom::fill(&mut nonce_bytes).unwrap();
    let alice_nonce = URL_SAFE_NO_PAD.encode(nonce_bytes);
    getrandom::fill(&mut nonce_bytes).unwrap();
    let bob_nonce = URL_SAFE_NO_PAD.encode(nonce_bytes);
    let first = provider(&path);
    let alice_tx = first
        .begin(
            client.client_id().as_str(),
            redirect,
            scopes.clone(),
            "state-alice-123",
            &alice_nonce,
            &challenge,
            alice,
            alice_session,
            UnixTimestamp::from_seconds(110),
        )
        .unwrap();
    let bob_tx = first
        .begin(
            client.client_id().as_str(),
            redirect,
            scopes.clone(),
            "state-bob-123",
            &bob_nonce,
            &challenge,
            bob,
            bob_session,
            UnixTimestamp::from_seconds(110),
        )
        .unwrap();
    assert_eq!(
        first
            .approve(
                &alice_tx,
                client.client_id().as_str(),
                scopes.clone(),
                bob,
                bob_session,
                UnixTimestamp::from_seconds(111)
            )
            .err(),
        Some(OidcProtocolError::AccessDenied),
    );
    let alice_code = first
        .approve(
            &alice_tx,
            client.client_id().as_str(),
            scopes.clone(),
            alice,
            alice_session,
            UnixTimestamp::from_seconds(111),
        )
        .unwrap();
    let bob_code = first
        .approve(
            &bob_tx,
            client.client_id().as_str(),
            scopes,
            bob,
            bob_session,
            UnixTimestamp::from_seconds(111),
        )
        .unwrap();
    assert_ne!(alice_code.subject().value(), bob_code.subject().value());
    drop(first);

    let restarted = provider(&path);
    assert_eq!(
        restarted
            .exchange_code(
                client.client_id().as_str(),
                None,
                alice_code.code(),
                redirect,
                "wrong-verifier",
                UnixTimestamp::from_seconds(112)
            )
            .err(),
        Some(OidcProtocolError::InvalidGrant),
    );
    let alice_tokens = restarted
        .exchange_code(
            client.client_id().as_str(),
            None,
            alice_code.code(),
            redirect,
            VERIFIER,
            UnixTimestamp::from_seconds(113),
        )
        .unwrap()
        .as_json();
    let bob_tokens = restarted
        .exchange_code(
            client.client_id().as_str(),
            None,
            bob_code.code(),
            redirect,
            VERIFIER,
            UnixTimestamp::from_seconds(113),
        )
        .unwrap()
        .as_json();
    assert_eq!(
        restarted
            .exchange_code(
                client.client_id().as_str(),
                None,
                alice_code.code(),
                redirect,
                VERIFIER,
                UnixTimestamp::from_seconds(114)
            )
            .err(),
        Some(OidcProtocolError::InvalidGrant),
    );
    for (tokens, expected_nonce) in [(&alice_tokens, &alice_nonce), (&bob_tokens, &bob_nonce)] {
        let payload = tokens["id_token"]
            .as_str()
            .unwrap()
            .split('.')
            .nth(1)
            .unwrap();
        let claims: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).unwrap()).unwrap();
        assert_eq!(claims["nonce"].as_str(), Some(expected_nonce.as_str()));
    }
    let alice_access = alice_tokens["access_token"].as_str().unwrap();
    let bob_access = bob_tokens["access_token"].as_str().unwrap();
    let alice_info = restarted
        .userinfo(alice_access, UnixTimestamp::from_seconds(115))
        .unwrap();
    let bob_info = restarted
        .userinfo(bob_access, UnixTimestamp::from_seconds(115))
        .unwrap();
    assert_eq!(alice_info["name"], "Alice");
    assert_eq!(bob_info["name"], "Bob");
    assert_ne!(alice_info["sub"], bob_info["sub"]);
    assert_eq!(alice_info["sub"], alice_code.subject().value());
    assert_eq!(bob_info["sub"], bob_code.subject().value());

    let mut current = accounts.load().unwrap();
    current
        .revoke_sessions_for_user(&alice, UnixTimestamp::from_seconds(120))
        .unwrap();
    accounts.store(&current).unwrap();
    assert_eq!(
        restarted
            .userinfo(alice_access, UnixTimestamp::from_seconds(121))
            .err(),
        Some(OidcProtocolError::AccessDenied),
    );
    assert!(
        restarted
            .userinfo(bob_access, UnixTimestamp::from_seconds(121))
            .is_ok()
    );
    SqliteOidcClientRepository::open(&path)
        .unwrap()
        .revoke_for_installation(client.installation_id(), UnixTimestamp::from_seconds(130))
        .unwrap();
    assert_eq!(
        restarted
            .userinfo(bob_access, UnixTimestamp::from_seconds(131))
            .err(),
        Some(OidcProtocolError::InvalidGrant),
    );
}
