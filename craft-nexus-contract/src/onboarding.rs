#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol,
};

mod test;

const ONBOARD: Symbol = symbol_short!("ONBOARD");

/// User roles in the CraftNexus platform
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum UserRole {
    None = 0,      // User has not onboarded
    Buyer = 1,     // Can purchase items
    Artisan = 2,   // Can sell items and create escrow
    Admin = 3,     // Platform administrator
}

/// Onboarding status for users
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserProfile {
    pub address: Address,
    pub role: UserRole,
    pub username: String,
    pub registered_at: u64,
    pub is_verified: bool,
}

/// Contract configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OnboardingConfig {
    pub require_username: bool,
    pub min_username_length: u32,
    pub max_username_length: u32,
    pub platform_admin: Address,
}

#[contract]
pub struct OnboardingContract;

#[contractimpl]
impl OnboardingContract {
    /// Initialize the onboarding contract
    /// 
    /// # Arguments
    /// * `admin` - Platform administrator address
    pub fn initialize(env: Env, admin: Address) -> OnboardingConfig {
        // Only the deployer can initialize
        admin.require_auth();
        
        let config = OnboardingConfig {
            require_username: true,
            min_username_length: 3,
            max_username_length: 50,
            platform_admin: admin.clone(),
        };
        
        // Store the configuration
        env.storage()
            .persistent()
            .set(&Symbol::new(&env, "CONFIG"), &config);
        
        // Store admin as initial admin role
        let admin_profile = UserProfile {
            address: admin.clone(),
            role: UserRole::Admin,
            username: "admin".to_string(),
            registered_at: env.ledger().timestamp(),
            is_verified: true,
        };
        
        env.storage()
            .persistent()
            .set(&(ONBOARD, admin.clone()), &admin_profile);
        
        config
    }

    /// Onboard a new user to the platform
    /// 
    /// # Arguments
    /// * `user` - User's wallet address
    /// * `username` - Desired username
    /// * `role` - Desired role (Buyer or Artisan)
    /// 
    /// # Behavior
    /// - Validates user is not already onboarded
    /// - Validates username requirements
    /// - Updates user profile with role
    /// - Emits UserOnboarded event
    /// 
    /// # Reverts if
    /// - User already onboarded
    /// - Username too short or too long
    /// - Invalid role specified
    pub fn onboard_user(
        env: Env,
        user: Address,
        username: String,
        role: UserRole,
    ) -> UserProfile {
        user.require_auth();
        
        // Validate role is valid (only Buyer or Artisan for self-onboarding)
        assert!(
            role == UserRole::Buyer || role == UserRole::Artisan,
            "Invalid role: can only onboard as Buyer or Artisan"
        );
        
        // Get configuration
        let config: OnboardingConfig = env.storage()
            .persistent()
            .get(&Symbol::new(&env, "CONFIG"))
            .expect("Contract not initialized");
        
        // Validate username length
        let username_len = username.len() as u32;
        assert!(
            username_len >= config.min_username_length,
            "Username too short"
        );
        assert!(
            username_len <= config.max_username_length,
            "Username too long"
        );
        
        // Check if user already onboarded
        let existing: Option<UserProfile> = env.storage()
            .persistent()
            .get(&(ONBOARD, user.clone()));
        
        assert!(existing.is_none(), "User already onboarded");
        
        // Create user profile
        let profile = UserProfile {
            address: user.clone(),
            role,
            username: username.clone(),
            registered_at: env.ledger().timestamp(),
            is_verified: false, // Verification could be added later
        };
        
        // Store profile
        env.storage()
            .persistent()
            .set(&(ONBOARD, user.clone()), &profile);
        
        // Emit event
        env.events()
            .publish((Symbol::new(&env, "UserOnboarded"),), (&user, &username, &role));
        
        profile
    }

    /// Get user profile by address
    /// 
    /// # Arguments
    /// * `user` - User's wallet address
    /// 
    /// # Returns
    /// UserProfile if user exists, reverts otherwise
    pub fn get_user(env: Env, user: Address) -> UserProfile {
        env.storage()
            .persistent()
            .get(&(ONBOARD, user))
            .expect("User not found")
    }

    /// Check if user is onboarded
    /// 
    /// # Arguments
    /// * `user` - User's wallet address
    /// 
    /// # Returns
    /// true if user has onboarded, false otherwise
    pub fn is_onboarded(env: Env, user: Address) -> bool {
        env.storage()
            .persistent()
            .get(&(ONBOARD, user))
            .is_some()
    }

    /// Get user's role
    /// 
    /// # Arguments
    /// * `user` - User's wallet address
    /// 
    /// # Returns
    /// UserRole if user exists, UserRole::None otherwise
    pub fn get_user_role(env: Env, user: Address) -> UserRole {
        match env.storage()
            .persistent()
            .get::<(Symbol, Address), UserProfile>(&(ONBOARD, user)) 
        {
            Some(profile) => profile.role,
            None => UserRole::None,
        }
    }

    /// Update user role (admin only)
    /// 
    /// # Arguments
    /// * `user` - User's wallet address
    /// * `new_role` - New role to assign
    /// 
    /// # Behavior
    /// - Only admin can update roles
    /// - Updates user profile with new role
    /// - Emits RoleUpdated event
    /// 
    /// # Reverts if
    /// - Caller is not admin
    /// - User not found
    pub fn update_user_role(
        env: Env,
        user: Address,
        new_role: UserRole,
    ) -> UserProfile {
        // Get config to verify admin
        let config: OnboardingConfig = env.storage()
            .persistent()
            .get(&Symbol::new(&env, "CONFIG"))
            .expect("Contract not initialized");
        
        // Only admin can update roles
        config.platform_admin.require_auth();
        
        // Get existing profile
        let mut profile: UserProfile = env.storage()
            .persistent()
            .get(&(ONBOARD, user.clone()))
            .expect("User not found");
        
        // Update role
        let old_role = profile.role;
        profile.role = new_role;
        
        // Store updated profile
        env.storage()
            .persistent()
            .set(&(ONBOARD, user.clone()), &profile);
        
        // Emit event
        env.events()
            .publish((Symbol::new(&env, "RoleUpdated"),), (&user, &old_role, &new_role));
        
        profile
    }

    /// Verify user (admin only)
    /// 
    /// # Arguments
    /// * `user` - User's wallet address
    /// 
    /// # Behavior
    /// - Only admin can verify users
    /// - Sets user verification status to true
    /// - Emits UserVerified event
    /// 
    /// # Reverts if
    /// - Caller is not admin
    /// - User not found
    pub fn verify_user(env: Env, user: Address) -> UserProfile {
        // Get config to verify admin
        let config: OnboardingConfig = env.storage()
            .persistent()
            .get(&Symbol::new(&env, "CONFIG"))
            .expect("Contract not initialized");
        
        // Only admin can verify users
        config.platform_admin.require_auth();
        
        // Get existing profile
        let mut profile: UserProfile = env.storage()
            .persistent()
            .get(&(ONBOARD, user.clone()))
            .expect("User not found");
        
        // Set verified
        profile.is_verified = true;
        
        // Store updated profile
        env.storage()
            .persistent()
            .set(&(ONBOARD, user.clone()), &profile);
        
        // Emit event
        env.events()
            .publish((Symbol::new(&env, "UserVerified"),), &user);
        
        profile
    }

    /// Get onboarding configuration
    /// 
    /// # Returns
    /// OnboardingConfig struct
    pub fn get_config(env: Env) -> OnboardingConfig {
        env.storage()
            .persistent()
            .get(&Symbol::new(&env, "CONFIG"))
            .expect("Contract not initialized")
    }

    /// Check if user has specific role
    /// 
    /// # Arguments
    /// * `user` - User's wallet address
    /// * `role` - Role to check
    /// 
    /// # Returns
    /// true if user has the specified role, false otherwise
    pub fn has_role(env: Env, user: Address, role: UserRole) -> bool {
        Self::get_user_role(env, user) == role
    }

    /// Check if user is verified
    /// 
    /// # Arguments
    /// * `user` - User's wallet address
    /// 
    /// # Returns
    /// true if user is verified, false otherwise
    pub fn is_verified(env: Env, user: Address) -> bool {
        match env.storage()
            .persistent()
            .get::<(Symbol, Address), UserProfile>(&(ONBOARD, user)) 
        {
            Some(profile) => profile.is_verified,
            None => false,
        }
    }
}
