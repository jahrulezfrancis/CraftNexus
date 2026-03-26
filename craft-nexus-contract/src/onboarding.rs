use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Symbol};

#[cfg(test)]
#[path = "onboarding_test.rs"]
mod onboarding_test;

/// Storage keys for the onboarding contract
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Maps a user address to their profile
    UserProfile(Address),
    /// Maps a normalized username to the owning address (uniqueness index)
    Username(String),
    /// Contract configuration
    Config,
}

/// User roles in the CraftNexus platform
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum UserRole {
    None = 0,    // User has not onboarded
    Buyer = 1,   // Can purchase items
    Artisan = 2, // Can sell items and create escrow
    Admin = 3,   // Platform administrator
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

/// Normalize a username to lowercase ASCII.
///
/// Iterates over each byte of the string and converts uppercase ASCII
/// letters (A-Z) to lowercase (a-z). Leading and trailing spaces are
/// stripped. This ensures case-insensitive uniqueness.
fn normalize_username(env: &Env, username: &String) -> String {
    let len = username.len() as usize;
    const MAX_INPUT_BYTES: usize = 256;
    if len > MAX_INPUT_BYTES {
        panic!("Username too long");
    }

    let mut buf = [0u8; MAX_INPUT_BYTES];
    username.copy_into_slice(&mut buf[0..len]);

    let mut out_len = 0;
    for i in 0..len {
        let b = buf[i];
        if b != b' ' {
            if b >= b'A' && b <= b'Z' {
                buf[out_len] = b + 32;
            } else {
                buf[out_len] = b;
            }
            out_len += 1;
        }
    }

    if out_len == 0 {
        return String::from_str(env, "");
    }

    let s = core::str::from_utf8(&buf[0..out_len]).unwrap_or("");
    String::from_str(env, s)
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
            .set(&DataKey::Config, &config);
        env.storage().persistent().extend_ttl(&DataKey::Config, 1000, 518400);

        let admin_username = String::from_str(&env, "admin");
        let normalized = normalize_username(&env, &admin_username);

        // Store admin as initial admin role
        let admin_profile = UserProfile {
            address: admin.clone(),
            role: UserRole::Admin,
            username: normalized.clone(),
            registered_at: env.ledger().timestamp(),
            is_verified: true,
        };

        env.storage()
            .persistent()
            .set(&DataKey::UserProfile(admin.clone()), &admin_profile);
        env.storage().persistent().extend_ttl(&DataKey::UserProfile(admin.clone()), 1000, 518400);

        // Reserve the "admin" username
        env.storage()
            .persistent()
            .set(&DataKey::Username(normalized.clone()), &admin);
        env.storage().persistent().extend_ttl(&DataKey::Username(normalized), 1000, 518400);

        config
    }

    /// Onboard a new user to the platform
    ///
    /// # Arguments
    /// * `user` - User's wallet address
    /// * `username` - Desired username
    /// * `role` - Desired role (Buyer or Artisan)
    ///
    /// # Reverts if
    /// - User already onboarded
    /// - Username already taken (case-insensitive)
    /// - Username too short or too long
    /// - Invalid role specified
    pub fn onboard_user(env: Env, user: Address, username: String, role: UserRole) -> UserProfile {
        user.require_auth();

        // Validate role is valid (only Buyer or Artisan for self-onboarding)
        assert!(
            role == UserRole::Buyer || role == UserRole::Artisan,
            "Invalid role: can only onboard as Buyer or Artisan"
        );

        // Get configuration
        let config: OnboardingConfig = env
            .storage()
            .persistent()
            .get(&DataKey::Config)
            .expect("Contract not initialized");
        env.storage().persistent().extend_ttl(&DataKey::Config, 1000, 518400);

        // Normalize the username (lowercase + trim whitespace)
        let normalized = normalize_username(&env, &username);

        // Validate normalized username length
        let username_len = normalized.len() as u32;
        assert!(
            username_len >= config.min_username_length,
            "Username too short"
        );
        assert!(
            username_len <= config.max_username_length,
            "Username too long"
        );

        // Check if user already onboarded
        let existing: Option<UserProfile> = env
            .storage()
            .persistent()
            .get(&DataKey::UserProfile(user.clone()));
        if existing.is_some() {
            env.storage().persistent().extend_ttl(&DataKey::UserProfile(user.clone()), 1000, 518400);
        }

        assert!(existing.is_none(), "User already onboarded");

        // Check username uniqueness
        assert!(
            !env.storage()
                .persistent()
                .has(&DataKey::Username(normalized.clone())),
            "Username already taken"
        );

        // Create user profile with normalized username
        let profile = UserProfile {
            address: user.clone(),
            role,
            username: normalized.clone(),
            registered_at: env.ledger().timestamp(),
            is_verified: false,
        };

        // Store profile
        env.storage()
            .persistent()
            .set(&DataKey::UserProfile(user.clone()), &profile);
        env.storage().persistent().extend_ttl(&DataKey::UserProfile(user.clone()), 1000, 518400);

        // Store username → address mapping for uniqueness enforcement
        env.storage()
            .persistent()
            .set(&DataKey::Username(normalized.clone()), &user);
        env.storage().persistent().extend_ttl(&DataKey::Username(normalized), 1000, 518400);

        // Emit event
        env.events()
            .publish((Symbol::new(&env, "UserOnboarded"),), &user);

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
        let profile: UserProfile = env.storage()
            .persistent()
            .get(&DataKey::UserProfile(user.clone()))
            .expect("User not found");
        env.storage().persistent().extend_ttl(&DataKey::UserProfile(user), 1000, 518400);
        profile
    }

    /// Get user profile by username (case-insensitive)
    ///
    /// # Arguments
    /// * `username` - Username to look up
    ///
    /// # Returns
    /// UserProfile if username exists, reverts otherwise
    pub fn get_user_by_username(env: Env, username: String) -> UserProfile {
        let normalized = normalize_username(&env, &username);

        let owner: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Username(normalized.clone()))
            .expect("Username not found");
        env.storage().persistent().extend_ttl(&DataKey::Username(normalized), 1000, 518400);

        let profile: UserProfile = env.storage()
            .persistent()
            .get(&DataKey::UserProfile(owner.clone()))
            .expect("User not found");
        env.storage().persistent().extend_ttl(&DataKey::UserProfile(owner), 1000, 518400);
        
        profile
    }

    /// Check if a username is already taken (case-insensitive)
    ///
    /// # Arguments
    /// * `username` - Username to check
    ///
    /// # Returns
    /// true if username is taken, false if available
    pub fn is_username_taken(env: Env, username: String) -> bool {
        let normalized = normalize_username(&env, &username);
        let has = env.storage()
            .persistent()
            .has(&DataKey::Username(normalized.clone()));
        if has {
            env.storage().persistent().extend_ttl(&DataKey::Username(normalized), 1000, 518400);
        }
        has
    }

    /// Check if user is onboarded
    ///
    /// # Arguments
    /// * `user` - User's wallet address
    ///
    /// # Returns
    /// true if user has onboarded, false otherwise
    pub fn is_onboarded(env: Env, user: Address) -> bool {
        let has = env.storage()
            .persistent()
            .get::<DataKey, UserProfile>(&DataKey::UserProfile(user.clone()))
            .is_some();
        if has {
            env.storage().persistent().extend_ttl(&DataKey::UserProfile(user), 1000, 518400);
        }
        has
    }

    /// Get user's role
    ///
    /// # Arguments
    /// * `user` - User's wallet address
    ///
    /// # Returns
    /// UserRole if user exists, UserRole::None otherwise
    pub fn get_user_role(env: Env, user: Address) -> UserRole {
        match env
            .storage()
            .persistent()
            .get::<DataKey, UserProfile>(&DataKey::UserProfile(user.clone()))
        {
            Some(profile) => {
                env.storage().persistent().extend_ttl(&DataKey::UserProfile(user), 1000, 518400);
                profile.role
            },
            None => UserRole::None,
        }
    }

    /// Update user role (admin only)
    ///
    /// # Arguments
    /// * `user` - User's wallet address
    /// * `new_role` - New role to assign
    ///
    /// # Reverts if
    /// - Caller is not admin
    /// - User not found
    pub fn update_user_role(env: Env, user: Address, new_role: UserRole) -> UserProfile {
        // Get config to verify admin
        let config: OnboardingConfig = env
            .storage()
            .persistent()
            .get(&DataKey::Config)
            .expect("Contract not initialized");
        env.storage().persistent().extend_ttl(&DataKey::Config, 1000, 518400);

        // Only admin can update roles
        config.platform_admin.require_auth();

        // Get existing profile
        let mut profile: UserProfile = env
            .storage()
            .persistent()
            .get(&DataKey::UserProfile(user.clone()))
            .expect("User not found");
        env.storage().persistent().extend_ttl(&DataKey::UserProfile(user.clone()), 1000, 518400);

        // Update role
        let _old_role = profile.role;
        profile.role = new_role;

        // Store updated profile
        env.storage()
            .persistent()
            .set(&DataKey::UserProfile(user.clone()), &profile);
        env.storage().persistent().extend_ttl(&DataKey::UserProfile(user.clone()), 1000, 518400);

        // Emit event
        env.events()
            .publish((Symbol::new(&env, "RoleUpdated"),), &user);

        profile
    }

    /// Verify user (admin only)
    ///
    /// # Arguments
    /// * `user` - User's wallet address
    ///
    /// # Reverts if
    /// - Caller is not admin
    /// - User not found
    pub fn verify_user(env: Env, user: Address) -> UserProfile {
        // Get config to verify admin
        let config: OnboardingConfig = env
            .storage()
            .persistent()
            .get(&DataKey::Config)
            .expect("Contract not initialized");
        env.storage().persistent().extend_ttl(&DataKey::Config, 1000, 518400);

        // Only admin can verify users
        config.platform_admin.require_auth();

        // Get existing profile
        let mut profile: UserProfile = env
            .storage()
            .persistent()
            .get(&DataKey::UserProfile(user.clone()))
            .expect("User not found");
        env.storage().persistent().extend_ttl(&DataKey::UserProfile(user.clone()), 1000, 518400);

        // Set verified
        profile.is_verified = true;

        // Store updated profile
        env.storage()
            .persistent()
            .set(&DataKey::UserProfile(user.clone()), &profile);
        env.storage().persistent().extend_ttl(&DataKey::UserProfile(user.clone()), 1000, 518400);

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
        let config: OnboardingConfig = env.storage()
            .persistent()
            .get(&DataKey::Config)
            .expect("Contract not initialized");
        env.storage().persistent().extend_ttl(&DataKey::Config, 1000, 518400);
        config
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
        match env
            .storage()
            .persistent()
            .get::<DataKey, UserProfile>(&DataKey::UserProfile(user.clone()))
        {
            Some(profile) => {
                env.storage().persistent().extend_ttl(&DataKey::UserProfile(user), 1000, 518400);
                profile.is_verified
            },
            None => false,
        }
    }
}
