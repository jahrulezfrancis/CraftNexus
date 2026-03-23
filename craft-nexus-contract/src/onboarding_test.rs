#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, token};

fn setup_test(env: &Env) -> (OnboardingContractClient<'static>, Address) {
    let contract_id = env.register_contract(None, OnboardingContract);
    let client = OnboardingContractClient::new(env, &contract_id);
    
    let admin = Address::generate(env);
    client.initialize(&admin);
    
    (client, admin)
}

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
fn test_onboard_user_as_buyer() {
    let env = Env::default();
    env.mock_all_auths();
    
    let (client, _) = setup_test(&env);
    
    let user = Address::generate(&env);
    let username = "john_doe";
    
    let profile = client.onboard_user(&user, &username.to_string(), &UserRole::Buyer);
    
    assert_eq!(profile.address, user);
    assert_eq!(profile.username, username);
    assert_eq!(profile.role, UserRole::Buyer);
    assert!(!profile.is_verified);
}

#[test]
fn test_onboard_user_as_artisan() {
    let env = Env::default();
    env.mock_all_auths();
    
    let (client, _) = setup_test(&env);
    
    let user = Address::generate(&env);
    let username = "artisan_jane";
    
    let profile = client.onboard_user(&user, &username.to_string(), &UserRole::Artisan);
    
    assert_eq!(profile.address, user);
    assert_eq!(profile.username, username);
    assert_eq!(profile.role, UserRole::Artisan);
}

#[test]
#[should_panic(expected = "User already onboarded")]
fn test_onboard_duplicate_user() {
    let env = Env::default();
    env.mock_all_auths();
    
    let (client, _) = setup_test(&env);
    
    let user = Address::generate(&env);
    let username = "test_user";
    
    client.onboard_user(&user, &username.to_string(), &UserRole::Buyer);
    client.onboard_user(&user, &username.to_string(), &UserRole::Artisan); // Should panic
}

#[test]
#[should_panic(expected = "Username too short")]
fn test_onboard_username_too_short() {
    let env = Env::default();
    env.mock_all_auths();
    
    let (client, _) = setup_test(&env);
    
    let user = Address::generate(&env);
    
    client.onboard_user(&user, &"ab".to_string(), &UserRole::Buyer); // Should panic
}

#[test]
#[should_panic(expected = "Username too long")]
fn test_onboard_username_too_long() {
    let env = Env::default();
    env.mock_all_auths();
    
    let (client, _) = setup_test(&env);
    
    let user = Address::generate(&env);
    let long_username = "a".to_string().repeat(100);
    
    client.onboard_user(&user, &long_username, &UserRole::Buyer); // Should panic
}

#[test]
#[should_panic(expected = "Invalid role: can only onboard as Buyer or Artisan")]
fn test_onboard_invalid_role() {
    let env = Env::default();
    env.mock_all_auths();
    
    let (client, _) = setup_test(&env);
    
    let user = Address::generate(&env);
    
    client.onboard_user(&user, &"test".to_string(), &UserRole::Admin); // Should panic
}

#[test]
fn test_get_user() {
    let env = Env::default();
    env.mock_all_auths();
    
    let (client, _) = setup_test(&env);
    
    let user = Address::generate(&env);
    let username = "test_user";
    
    client.onboard_user(&user, &username.to_string(), &UserRole::Buyer);
    
    let profile = client.get_user(&user);
    
    assert_eq!(profile.username, username);
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
    
    client.onboard_user(&user, &"test".to_string(), &UserRole::Buyer);
    
    assert!(client.is_onboarded(&user));
}

#[test]
fn test_get_user_role() {
    let env = Env::default();
    env.mock_all_auths();
    
    let (client, _) = setup_test(&env);
    
    let buyer = Address::generate(&env);
    let artisan = Address::generate(&env);
    
    client.onboard_user(&buyer, &"buyer_user".to_string(), &UserRole::Buyer);
    client.onboard_user(&artisan, &"artisan_user".to_string(), &UserRole::Artisan);
    
    assert_eq!(client.get_user_role(&buyer), UserRole::Buyer);
    assert_eq!(client.get_user_role(&artisan), UserRole::Artisan);
    assert_eq!(client.get_user_role(&Address::generate(&env)), UserRole::None);
}

#[test]
fn test_update_user_role() {
    let env = Env::default();
    env.mock_all_auths();
    
    let (client, admin) = setup_test(&env);
    
    let user = Address::generate(&env);
    client.onboard_user(&user, &"test_user".to_string(), &UserRole::Buyer);
    
    let updated = client.update_user_role(&user, &UserRole::Artisan);
    
    assert_eq!(updated.role, UserRole::Artisan);
}

#[test]
fn test_verify_user() {
    let env = Env::default();
    env.mock_all_auths();
    
    let (client, _) = setup_test(&env);
    
    let user = Address::generate(&env);
    client.onboard_user(&user, &"test_user".to_string(), &UserRole::Artisan);
    
    let verified = client.verify_user(&user);
    
    assert!(verified.is_verified);
}

#[test]
fn test_has_role() {
    let env = Env::default();
    env.mock_all_auths();
    
    let (client, _) = setup_test(&env);
    
    let user = Address::generate(&env);
    client.onboard_user(&user, &"test_user".to_string(), &UserRole::Artisan);
    
    assert!(client.has_role(&user, &UserRole::Artisan));
    assert!(!client.has_role(&user, &UserRole::Buyer));
}

#[test]
fn test_is_verified() {
    let env = Env::default();
    env.mock_all_auths();
    
    let (client, _) = setup_test(&env);
    
    let user = Address::generate(&env);
    client.onboard_user(&user, &"test_user".to_string(), &UserRole::Artisan);
    
    assert!(!client.is_verified(&user));
    
    client.verify_user(&user);
    
    assert!(client.is_verified(&user));
}
