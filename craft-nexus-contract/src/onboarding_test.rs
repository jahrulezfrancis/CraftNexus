#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup_test(env: &Env) -> (OnboardingContractClient<'static>, Address) {
    let contract_id = env.register_contract(None, OnboardingContract);
    let client = OnboardingContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    client.initialize(&admin);

    (client, admin)
}

// ===== Initialization =====

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin) = setup_test(&env);
    let config = client.get_config();

    assert_eq!(config.platform_admin, admin);
    assert_eq!(config.min_username_length, 3);
    assert_eq!(config.max_username_length, 50);
}

#[test]
fn test_initialize_reserves_admin_username() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    // "admin" should already be taken
    assert!(client.is_username_taken(&String::from_str(&env, "admin")));
    assert!(client.is_username_taken(&String::from_str(&env, "ADMIN")));
    assert!(client.is_username_taken(&String::from_str(&env, "Admin")));
}

// ===== Onboarding =====

#[test]
fn test_onboard_user_as_buyer() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user = Address::generate(&env);
    let username = String::from_str(&env, "john_doe");

    let profile = client.onboard_user(&user, &username, &UserRole::Buyer);

    assert_eq!(profile.address, user);
    assert_eq!(profile.username, String::from_str(&env, "john_doe"));
    assert_eq!(profile.role, UserRole::Buyer);
    assert!(!profile.is_verified);
}

#[test]
fn test_onboard_user_as_artisan() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user = Address::generate(&env);
    let username = String::from_str(&env, "artisan_jane");

    let profile = client.onboard_user(&user, &username, &UserRole::Artisan);

    assert_eq!(profile.address, user);
    assert_eq!(profile.username, String::from_str(&env, "artisan_jane"));
    assert_eq!(profile.role, UserRole::Artisan);
}

#[test]
fn test_onboard_stores_normalized_username() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user = Address::generate(&env);
    let username = String::from_str(&env, "JohnDoe");

    let profile = client.onboard_user(&user, &username, &UserRole::Buyer);

    // Username should be stored as lowercase
    assert_eq!(profile.username, String::from_str(&env, "johndoe"));
}

#[test]
#[should_panic(expected = "User already onboarded")]
fn test_onboard_duplicate_user() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user = Address::generate(&env);
    let username1 = String::from_str(&env, "test_user");
    let username2 = String::from_str(&env, "other_name");

    client.onboard_user(&user, &username1, &UserRole::Buyer);
    client.onboard_user(&user, &username2, &UserRole::Artisan); // Should panic
}

#[test]
#[should_panic(expected = "Username too short")]
fn test_onboard_username_too_short() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user = Address::generate(&env);
    let username = String::from_str(&env, "ab");

    client.onboard_user(&user, &username, &UserRole::Buyer); // Should panic
}

#[test]
#[should_panic(expected = "Username too long")]
fn test_onboard_username_too_long() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user = Address::generate(&env);
    // 51 character username (max is 50)
    let long_username =
        String::from_str(&env, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");

    client.onboard_user(&user, &long_username, &UserRole::Buyer); // Should panic
}

#[test]
#[should_panic(expected = "Invalid role: can only onboard as Buyer or Artisan")]
fn test_onboard_invalid_role() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user = Address::generate(&env);
    let username = String::from_str(&env, "test");

    client.onboard_user(&user, &username, &UserRole::Admin); // Should panic
}

// ===== Username Uniqueness =====

#[test]
#[should_panic(expected = "Username already taken")]
fn test_onboard_duplicate_username_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let username = String::from_str(&env, "craftsman");

    client.onboard_user(&user1, &username, &UserRole::Buyer);
    client.onboard_user(&user2, &username, &UserRole::Artisan); // Should panic
}

#[test]
#[should_panic(expected = "Username already taken")]
fn test_onboard_duplicate_username_case_insensitive() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.onboard_user(&user1, &String::from_str(&env, "Alice"), &UserRole::Buyer);
    // "alice" should match "Alice" after normalization
    client.onboard_user(&user2, &String::from_str(&env, "alice"), &UserRole::Artisan);
    // Should panic
}

#[test]
#[should_panic(expected = "Username already taken")]
fn test_onboard_duplicate_username_mixed_case() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.onboard_user(
        &user1,
        &String::from_str(&env, "CraftMaster"),
        &UserRole::Buyer,
    );
    client.onboard_user(
        &user2,
        &String::from_str(&env, "CRAFTMASTER"),
        &UserRole::Artisan,
    ); // Should panic
}

// ===== Username Lookup =====

#[test]
fn test_get_user_by_username() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user = Address::generate(&env);
    let username = String::from_str(&env, "craft_user");

    client.onboard_user(&user, &username, &UserRole::Buyer);

    let profile = client.get_user_by_username(&username);
    assert_eq!(profile.address, user);
    assert_eq!(profile.username, String::from_str(&env, "craft_user"));
}

#[test]
fn test_get_user_by_username_case_insensitive() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user = Address::generate(&env);
    client.onboard_user(&user, &String::from_str(&env, "john_doe"), &UserRole::Buyer);

    // Should find user regardless of case
    let profile = client.get_user_by_username(&String::from_str(&env, "JOHN_DOE"));
    assert_eq!(profile.address, user);

    let profile2 = client.get_user_by_username(&String::from_str(&env, "John_Doe"));
    assert_eq!(profile2.address, user);
}

#[test]
#[should_panic(expected = "Username not found")]
fn test_get_user_by_username_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    client.get_user_by_username(&String::from_str(&env, "nonexistent"));
}

// ===== Username Availability =====

#[test]
fn test_is_username_taken() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user = Address::generate(&env);
    let username = String::from_str(&env, "craft_user");

    // Before registration
    assert!(!client.is_username_taken(&username));

    client.onboard_user(&user, &username, &UserRole::Buyer);

    // After registration
    assert!(client.is_username_taken(&username));
    // Case-insensitive check
    assert!(client.is_username_taken(&String::from_str(&env, "CRAFT_USER")));
    assert!(client.is_username_taken(&String::from_str(&env, "Craft_User")));
    // Different username should be available
    assert!(!client.is_username_taken(&String::from_str(&env, "other_user")));
}

// ===== Existing Feature Tests =====

#[test]
fn test_get_user() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user = Address::generate(&env);
    let username = String::from_str(&env, "test_user");

    client.onboard_user(&user, &username, &UserRole::Buyer);

    let profile = client.get_user(&user);
    assert_eq!(profile.username, String::from_str(&env, "test_user"));
}

#[test]
#[should_panic(expected = "User not found")]
fn test_get_user_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user = Address::generate(&env);
    client.get_user(&user); // Should panic
}

#[test]
fn test_is_onboarded() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user = Address::generate(&env);

    assert!(!client.is_onboarded(&user));

    client.onboard_user(&user, &String::from_str(&env, "test"), &UserRole::Buyer);

    assert!(client.is_onboarded(&user));
}

#[test]
fn test_get_user_role() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let buyer = Address::generate(&env);
    let artisan = Address::generate(&env);

    client.onboard_user(
        &buyer,
        &String::from_str(&env, "buyer_user"),
        &UserRole::Buyer,
    );
    client.onboard_user(
        &artisan,
        &String::from_str(&env, "artisan_user"),
        &UserRole::Artisan,
    );

    assert_eq!(client.get_user_role(&buyer), UserRole::Buyer);
    assert_eq!(client.get_user_role(&artisan), UserRole::Artisan);
    assert_eq!(
        client.get_user_role(&Address::generate(&env)),
        UserRole::None
    );
}

#[test]
fn test_update_user_role() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin) = setup_test(&env);

    let user = Address::generate(&env);
    client.onboard_user(
        &user,
        &String::from_str(&env, "test_user"),
        &UserRole::Buyer,
    );

    let updated = client.update_user_role(&user, &UserRole::Artisan);
    assert_eq!(updated.role, UserRole::Artisan);
}

#[test]
fn test_verify_user() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user = Address::generate(&env);
    client.onboard_user(
        &user,
        &String::from_str(&env, "test_user"),
        &UserRole::Artisan,
    );

    let verified = client.verify_user(&user);
    assert!(verified.is_verified);
}

#[test]
fn test_has_role() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user = Address::generate(&env);
    client.onboard_user(
        &user,
        &String::from_str(&env, "test_user"),
        &UserRole::Artisan,
    );

    assert!(client.has_role(&user, &UserRole::Artisan));
    assert!(!client.has_role(&user, &UserRole::Buyer));
}

#[test]
fn test_is_verified() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_test(&env);

    let user = Address::generate(&env);
    client.onboard_user(
        &user,
        &String::from_str(&env, "test_user"),
        &UserRole::Artisan,
    );

    assert!(!client.is_verified(&user));

    client.verify_user(&user);

    assert!(client.is_verified(&user));
}
