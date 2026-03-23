#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env, token};

fn setup_test(env: &Env) -> (EscrowContractClient<'static>, Address, Address, Address, token::StellarAssetClient<'static>, Address) {
    let contract_id = env.register_contract(None, EscrowContract);
    let client = EscrowContractClient::new(env, &contract_id);

    let buyer = Address::generate(env);
    let seller = Address::generate(env);
    let platform_wallet = Address::generate(env);
    let admin = Address::generate(env);
    
    let token_admin = Address::generate(env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_admin_client = token::StellarAssetClient::new(env, &token_contract.address());

    // Initialize contract with platform config
    client.__init(&platform_wallet, &admin, &500);

    (client, buyer, seller, token_contract.address(), token_admin_client, platform_wallet)
}

#[test]
fn test_create_escrow_success() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, buyer, seller, token_id, token_admin, _) = setup_test(&env);
    
    token_admin.mint(&buyer, &1000);
    
    let order_id = 1;
    let amount = 500;
    let window = 3600;
    
    let escrow = client.create_escrow(&buyer, &seller, &token_id, &amount, &order_id, &Some(window));
    
    assert_eq!(escrow.buyer, buyer);
    assert_eq!(escrow.seller, seller);
    assert_eq!(escrow.amount, amount);
    assert_eq!(escrow.status, EscrowStatus::Pending);
    assert_eq!(escrow.release_window, window);
    
    let stored_escrow = client.get_escrow(&order_id);
    assert_eq!(stored_escrow, escrow);
}

#[test]
fn test_create_escrow_default_window() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, buyer, seller, token_id, token_admin, _) = setup_test(&env);
    
    token_admin.mint(&buyer, &1000);
    let escrow = client.create_escrow(&buyer, &seller, &token_id, &500, &1, &None);
    
    assert_eq!(escrow.release_window, 604800); // 7 days
}

#[test]
fn test_release_funds_success() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, buyer, seller, token_id, token_admin, platform_wallet) = setup_test(&env);
    
    token_admin.mint(&buyer, &1000);
    client.create_escrow(&buyer, &seller, &token_id, &500, &1, &None);
    
    client.release_funds(&1);
    
    let escrow = client.get_escrow(&1);
    assert_eq!(escrow.status, EscrowStatus::Released);
    
    let token_client = token::Client::new(&env, &token_id);
    // Seller receives 500 - 25 (5% fee) = 475
    assert_eq!(token_client.balance(&seller), 475);
    // Platform receives 25 (5% fee)
    assert_eq!(token_client.balance(&platform_wallet), 25);
    assert_eq!(token_client.balance(&client.address), 0);
    
    // Check total fees collected
    assert_eq!(client.get_total_fees_collected(), 25);
}

#[test]
#[should_panic(expected = "Escrow already processed")]
fn test_release_funds_already_processed() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, buyer, seller, token_id, token_admin, _) = setup_test(&env);
    
    token_admin.mint(&buyer, &1000);
    client.create_escrow(&buyer, &seller, &token_id, &500, &1, &None);
    client.release_funds(&1);
    client.release_funds(&1); // Should panic
}

#[test]
fn test_auto_release_success_after_window() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, buyer, seller, token_id, token_admin, platform_wallet) = setup_test(&env);
    
    token_admin.mint(&buyer, &1000);
    let window = 100;
    client.create_escrow(&buyer, &seller, &token_id, &500, &1, &Some(window));
    
    // Advance time
    env.ledger().with_mut(|li| {
        li.timestamp += window + 1;
    });
    
    assert!(client.can_auto_release(&1));
    client.auto_release(&1);
    
    let escrow = client.get_escrow(&1);
    assert_eq!(escrow.status, EscrowStatus::Released);
    
    let token_client = token::Client::new(&env, &token_id);
    // Seller receives 500 - 25 (5% fee) = 475
    assert_eq!(token_client.balance(&seller), 475);
    // Platform receives 25 (5% fee)
    assert_eq!(token_client.balance(&platform_wallet), 25);
}

#[test]
#[should_panic(expected = "Release window not yet elapsed")]
fn test_auto_release_failure_before_window() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, buyer, seller, token_id, token_admin, _) = setup_test(&env);
    
    token_admin.mint(&buyer, &1000);
    client.create_escrow(&buyer, &seller, &token_id, &500, &1, &Some(100));
    
    assert!(!client.can_auto_release(&1));
    client.auto_release(&1);
}

#[test]
fn test_refund_success_by_buyer() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, buyer, seller, token_id, token_admin, _) = setup_test(&env);
    
    token_admin.mint(&buyer, &1000);
    client.create_escrow(&buyer, &seller, &token_id, &500, &1, &None);
    
    client.refund(&1, &buyer);
    
    let escrow = client.get_escrow(&1);
    assert_eq!(escrow.status, EscrowStatus::Refunded);
    
    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&buyer), 1000);
}

#[test]
#[should_panic(expected = "Not authorized to refund")]
fn test_refund_failure_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, buyer, seller, token_id, token_admin, _) = setup_test(&env);
    
    token_admin.mint(&buyer, &1000);
    client.create_escrow(&buyer, &seller, &token_id, &500, &1, &None);
    
    let unauthorized = Address::generate(&env);
    client.refund(&1, &unauthorized);
}

#[test]
#[should_panic(expected = "Escrow not found")]
fn test_get_escrow_not_found() {
    let env = Env::default();
    let (client, _, _, _, _, _) = setup_test(&env);
    client.get_escrow(&999);
}

#[test]
#[should_panic(expected = "Amount must be positive")]
fn test_create_escrow_zero_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, buyer, seller, token_id, token_admin, _) = setup_test(&env);
    
    token_admin.mint(&buyer, &1000);
    client.create_escrow(&buyer, &seller, &token_id, &0, &1, &None);
}

#[test]
#[should_panic(expected = "Amount must be positive")]
fn test_create_escrow_negative_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, buyer, seller, token_id, token_admin, _) = setup_test(&env);
    
    token_admin.mint(&buyer, &1000);
    client.create_escrow(&buyer, &seller, &token_id, &-100, &1, &None);
}

#[test]
#[should_panic(expected = "Buyer and seller must be different")]
fn test_create_escrow_same_buyer_seller() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, buyer, _, token_id, token_admin, _) = setup_test(&env);
    
    token_admin.mint(&buyer, &1000);
    client.create_escrow(&buyer, &buyer, &token_id, &500, &1, &None);
}

// ===== Platform Fee Tests =====

#[test]
fn test_platform_fee_deduction_5_percent() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, buyer, seller, token_id, token_admin, platform_wallet) = setup_test(&env);
    
    token_admin.mint(&buyer, &10000);
    // Create escrow with 1000 (should have 50 fee at 5%)
    client.create_escrow(&buyer, &seller, &token_id, &1000, &1, &None);
    
    client.release_funds(&1);
    
    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&seller), 950);  // 1000 - 50
    assert_eq!(token_client.balance(&platform_wallet), 50);
    assert_eq!(client.get_total_fees_collected(), 50);
}

#[test]
fn test_platform_fee_deduction_10_percent() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, EscrowContract);
    let client = EscrowContractClient::new(&env, &contract_id);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let platform_wallet = Address::generate(&env);
    let admin = Address::generate(&env);
    
    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_admin_client = token::StellarAssetClient::new(&env, &token_contract.address());
    
    // Initialize with 10% fee
    client.__init(&platform_wallet, &admin, &1000);
    
    token_admin_client.mint(&buyer, &10000);
    client.create_escrow(&buyer, &seller, &token_contract.address(), &1000, &1, &None);
    
    client.release_funds(&1);
    
    let token_client = token::Client::new(&env, &token_contract.address());
    assert_eq!(token_client.balance(&seller), 900);  // 1000 - 100
    assert_eq!(token_client.balance(&platform_wallet), 100);
}

#[test]
fn test_calculate_fee_for_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _, _, _, _) = setup_test(&env);
    
    // 5% of 1000 = 50
    let fee = client.calculate_fee_for_amount(&1000);
    assert_eq!(fee, 50);
    
    // 5% of 500 = 25
    let fee = client.calculate_fee_for_amount(&500);
    assert_eq!(fee, 25);
}

#[test]
fn test_calculate_seller_net_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _, _, _, _) = setup_test(&env);
    
    // 1000 - 50 = 950
    let net = client.calculate_seller_net_amount(&1000);
    assert_eq!(net, 950);
    
    // 500 - 25 = 475
    let net = client.calculate_seller_net_amount(&500);
    assert_eq!(net, 475);
}

#[test]
fn test_update_platform_fee() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, EscrowContract);
    let client = EscrowContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let platform_wallet = Address::generate(&env);
    let seller = Address::generate(&env);
    
    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_admin_client = token::StellarAssetClient::new(&env, &token_contract.address());
    
    // Initialize with 5% fee
    client.__init(&platform_wallet, &admin, &500);
    
    // Get initial fee
    assert_eq!(client.get_platform_fee(), 500);
    
    // Update to 8% fee (800 bps) - admin auth required
    client.update_platform_fee(&800);
    
    assert_eq!(client.get_platform_fee(), 800);
    
    // Now create escrow and release - should use 8%
    token_admin_client.mint(&Address::generate(&env), &10000);
    let buyer = Address::generate(&env);
    token_admin_client.mint(&buyer, &1000);
    client.create_escrow(&buyer, &seller, &token_contract.address(), &1000, &1, &None);
    
    client.release_funds(&1);
    
    let token_client = token::Client::new(&env, &token_contract.address());
    // 1000 - 80 = 920
    assert_eq!(token_client.balance(&seller), 920);
    assert_eq!(token_client.balance(&platform_wallet), 80);
}

#[test]
#[should_panic(expected = "Fee too high")]
fn test_update_platform_fee_too_high() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, EscrowContract);
    let client = EscrowContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let platform_wallet = Address::generate(&env);
    
    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    
    // Initialize with 5% fee
    client.__init(&platform_wallet, &admin, &500);
    
    // Try to set fee above max (10%)
    client.update_platform_fee(&1500);
}

#[test]
fn test_total_fees_accumulate() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, buyer, seller, token_id, token_admin, platform_wallet) = setup_test(&env);
    
    token_admin.mint(&buyer, &3000);
    
    // Create and release multiple escrows
    client.create_escrow(&buyer, &seller, &token_id, &1000, &1, &None);
    client.release_funds(&1);
    
    client.create_escrow(&buyer, &seller, &token_id, &500, &2, &None);
    client.release_funds(&2);
    
    let token_client = token::Client::new(&env, &token_id);
    // Total fees: 50 + 25 = 75
    assert_eq!(token_client.balance(&platform_wallet), 75);
    assert_eq!(client.get_total_fees_collected(), 75);
}
