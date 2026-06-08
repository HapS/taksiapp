// Auth Service - User authentication operations
use crate::modules::auth::helpers::password::{
    hash_password, validate_password_strength, verify_password,
};
use crate::modules::auth::models::{
    PasswordReset, PasswordResetModel, SessionData, User, UserActiveModel, UserModel,
};
use crate::modules::ecommerce::models::{cart, cart_item, Cart, CartItem};
use anyhow::Result;
use chrono::Utc;
use sea_orm::*;
use serde_json::Value as JsonValue;

#[derive(Debug)]
pub enum AuthError {
    InvalidCredentials,
    UserNotFound,
    EmailAlreadyExists,
    EmailFormatInvalid,
    UserAlreadyExists,
    WeakPassword,
    DatabaseError(DbErr),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::InvalidCredentials => write!(f, "Invalid credentials"),
            AuthError::UserNotFound => write!(f, "User not found"),
            AuthError::EmailAlreadyExists => write!(f, "Email already exists"),
            AuthError::EmailFormatInvalid => write!(f, "Email format is invalid"),
            AuthError::UserAlreadyExists => write!(f, "User already exists"),
            AuthError::WeakPassword => write!(f, "Password is too weak"),
            AuthError::DatabaseError(e) => write!(f, "Veritabanı hatası: {}", e),
        }
    }
}

impl std::error::Error for AuthError {}

impl From<DbErr> for AuthError {
    fn from(err: DbErr) -> Self {
        AuthError::DatabaseError(err)
    }
}

/// Register a new user
pub async fn register(
    db: &DatabaseConnection,
    username: &str,
    email: &str,
    password: &str,
    first_name: Option<String>,
    last_name: Option<String>,
) -> Result<UserModel, AuthError> {
    // Validate password
    validate_password_strength(password).map_err(|_| AuthError::WeakPassword)?;

    // Check if email already exists
    let existing = User::find()
        .filter(crate::modules::auth::models::user::Column::Email.eq(email))
        .one(db)
        .await?;

    //duplicate email or username error response
    let existing_username = User::find()
        .filter(crate::modules::auth::models::user::Column::Username.eq(username))
        .one(db)
        .await?;

    if existing.is_some() {
        return Err(AuthError::EmailAlreadyExists);
    }
    if existing_username.is_some() {
        return Err(AuthError::UserAlreadyExists);
    }

    let email_format =
        regex::Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();

    if !email_format.is_match(email) {
        return Err(AuthError::EmailFormatInvalid);
    }

    // Hash password
    let password_hash = hash_password(password).map_err(|_| AuthError::WeakPassword)?;

    //is_guest kontrol

    // Create user
    let user = UserActiveModel {
        username: Set(username.to_string()),
        email: Set(email.to_string()),
        password: Set(Some(password_hash)),
        first_name: Set(first_name),
        last_name: Set(last_name),
        ..Default::default()
    };

    let user = user.insert(db).await?;
    Ok(user)
}

/// Register guest user
/// guest user sepete ürün atmışsa veya sepeti göndermişse veya favorilerine ürün eklemiş veya görüntülediğinde otomatik olarak oluşturulur.
/// kullanıcı kayıt olmak isteğinde eğer tarayıcısında session verileri duruyorsa kendisini tanıyorsak yeni bir kullanıcı değil var olan guest kullanıcısı gerçek bir kullanıcıya dönüştürülür.
/// her ne kadar register fonksiyonuna benziyor olsa da farklı muameleler çekmek istediğimizde register fonksiyonu çöpe dönmesin
pub async fn register_guest_user(
    db: &DatabaseConnection,
    user_id: i64,
    username: &str,
    email: &str,
    password: &str,
    first_name: Option<String>,
    last_name: Option<String>,
) -> Result<UserModel, AuthError> {
    // Validate password
    validate_password_strength(password).map_err(|_| AuthError::WeakPassword)?;

    // Check if email already exists
    let existing = User::find()
        .filter(crate::modules::auth::models::user::Column::Email.eq(email))
        .one(db)
        .await?;

    //duplicate email or username error response
    let existing_username = User::find()
        .filter(crate::modules::auth::models::user::Column::Username.eq(username))
        .one(db)
        .await?;

    if existing.is_some() {
        return Err(AuthError::EmailAlreadyExists);
    }
    if existing_username.is_some() {
        return Err(AuthError::UserAlreadyExists);
    }

    let email_format =
        regex::Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();

    if !email_format.is_match(email) {
        return Err(AuthError::EmailFormatInvalid);
    }

    // Hash password
    let password_hash = hash_password(password).map_err(|_| AuthError::WeakPassword)?;

    // Create user
    let user = UserActiveModel {
        id: Set(user_id),
        username: Set(username.to_string()),
        email: Set(email.to_string()),
        password: Set(Some(password_hash)),
        first_name: Set(first_name),
        last_name: Set(last_name),
        is_guest: Set(false),
        ..Default::default()
    };

    let user = user.update(db).await?;
    Ok(user)
}

/// Login user with username/email and password
pub async fn login(
    db: &DatabaseConnection,
    username_or_email: &str,
    password: &str,
    guest_user_id: Option<i64>,
) -> Result<SessionData, AuthError> {
    // Find user by username or email
    let user = User::find()
        .filter(
            sea_orm::Condition::any()
                .add(crate::modules::auth::models::user::Column::Username.eq(username_or_email))
                .add(crate::modules::auth::models::user::Column::Email.eq(username_or_email)),
        )
        .one(db)
        .await?
        .ok_or(AuthError::UserNotFound)?;

    // Verify password
    let user_password = user
        .password
        .as_ref()
        .ok_or(AuthError::InvalidCredentials)?;
    let is_valid =
        verify_password(password, user_password).map_err(|_| AuthError::InvalidCredentials)?;

    if !is_valid {
        return Err(AuthError::InvalidCredentials);
    }

    // Misafir kullanıcı sepetini birleştir (eğer varsa)
    if let Some(guest_id) = guest_user_id {
        if let Err(e) = merge_guest_cart_to_user(db, guest_id, user.id).await {
            eprintln!("Misafir sepet birleştirme hatası: {}", e);
            // Sepet birleştirme hatası olsa bile login işlemini başarısızlığa uğratma
        }
    }

    // Return session data with permissions
    user.to_session_data(db).await.map_err(|e| {
        eprintln!("Yetki yükleme hatası: {}", e);
        AuthError::DatabaseError(e)
    })
}

/// Misafir kullanıcı sepetini giriş yapmış kullanıcının sepetine birleştir
async fn merge_guest_cart_to_user(
    db: &DatabaseConnection,
    guest_user_id: i64,
    logged_user_id: i64,
) -> Result<(), AuthError> {
    // Misafir kullanıcının açık sepetini bul
    let guest_cart = Cart::find()
        .filter(cart::Column::UserId.eq(guest_user_id))
        .filter(cart::Column::Status.eq(cart::status::OPEN_CART))
        .one(db)
        .await?;

    let guest_cart = match guest_cart {
        Some(cart) => cart,
        None => return Ok(()), // Birleştirilecek misafir sepeti yok
    };

    // Giriş yapmış kullanıcının açık sepetini bul
    let user_cart = Cart::find()
        .filter(cart::Column::UserId.eq(logged_user_id))
        .filter(cart::Column::Status.eq(cart::status::OPEN_CART))
        .one(db)
        .await?;

    match user_cart {
        Some(user_cart) => {
            // Kullanıcının mevcut sepeti var, misafir ürünlerini ona birleştir
            let guest_items = CartItem::find()
                .filter(cart_item::Column::CartId.eq(guest_cart.id))
                .all(db)
                .await?;

            for guest_item in guest_items {
                // Aynı ürün+varyant kullanıcı sepetinde zaten var mı kontrol et
                let mut query = CartItem::find()
                    .filter(cart_item::Column::CartId.eq(user_cart.id))
                    .filter(cart_item::Column::ProductId.eq(guest_item.product_id));

                // Variant key karşılaştırması IS NULL
                query = match &guest_item.variant_key {
                    Some(key) => query.filter(cart_item::Column::VariantKey.eq(key.clone())),
                    None => query.filter(cart_item::Column::VariantKey.is_null()),
                };

                let existing_item = query.one(db).await?;

                if let Some(existing) = existing_item {
                    // Miktarı güncelle (aynı ürün+varyant varsa adetleri topla)
                    let existing_quantity = existing.quantity;
                    let new_quantity = existing_quantity + guest_item.quantity;

                    let mut active_item: cart_item::ActiveModel = existing.into();
                    active_item.quantity = Set(new_quantity);
                    active_item.updated_at = Set(Some(Utc::now().into()));
                    active_item.update(db).await?;
                } else {
                    // Ürünü kullanıcı sepetine taşı
                    let mut active_item: cart_item::ActiveModel = guest_item.into();
                    active_item.cart_id = Set(user_cart.id);
                    active_item.updated_at = Set(Some(Utc::now().into()));
                    active_item.update(db).await?;
                }
            }

            // Misafir sepetini sil
            let guest_cart_active: cart::ActiveModel = guest_cart.into();
            guest_cart_active.delete(db).await?;

            // Kullanıcı sepeti zaman damgasını güncelle
            let mut user_cart_active: cart::ActiveModel = user_cart.into();
            user_cart_active.updated_at = Set(Some(Utc::now().into()));
            user_cart_active.update(db).await?;
        }
        None => {
            // Kullanıcının sepeti yok, misafir sepetini kullanıcıya aktar
            let mut guest_cart_active: cart::ActiveModel = guest_cart.into();
            guest_cart_active.user_id = Set(logged_user_id);
            guest_cart_active.updated_at = Set(Some(Utc::now().into()));
            guest_cart_active.update(db).await?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub enum SocialProvider {
    Google,
    #[allow(dead_code)]
    Apple,
}

/// Misafir kullanıcıyı sosyal medya ile giriş yapan gerçek kullanıcıya dönüştür
async fn convert_guest_to_social_user(
    db: &DatabaseConnection,
    guest_user_id: i64,
    provider: SocialProvider,
    provider_id: &str,
    email: &str,
    first_name: Option<String>,
    last_name: Option<String>,
) -> Result<UserModel, AuthError> {
    // Guest user'ı bul
    let guest_user = User::find_by_id(guest_user_id)
        .one(db)
        .await?
        .ok_or(AuthError::UserNotFound)?;

    // Guest user değilse hata döndür
    if !guest_user.is_guest {
        return Err(AuthError::UserAlreadyExists);
    }

    // Email'in başka bir kullanıcıda olmadığını kontrol et
    let existing_email = User::find()
        .filter(crate::modules::auth::models::user::Column::Email.eq(email))
        .filter(crate::modules::auth::models::user::Column::Id.ne(guest_user_id))
        .one(db)
        .await?;

    if existing_email.is_some() {
        return Err(AuthError::EmailAlreadyExists);
    }

    // Username oluştur (email'den veya provider_id'den)
    let username_base = email.split('@').next().unwrap_or(provider_id);
    let mut final_username = username_base.to_string();
    let mut counter = 1;

    // Unique username bul
    while User::find()
        .filter(crate::modules::auth::models::user::Column::Username.eq(&final_username))
        .filter(crate::modules::auth::models::user::Column::Id.ne(guest_user_id))
        .one(db)
        .await?
        .is_some()
    {
        final_username = format!("{}_{}", username_base, counter);
        counter += 1;
    }

    // Guest user'ı güncelle
    let mut active_model: UserActiveModel = guest_user.into();
    active_model.username = Set(final_username);
    active_model.email = Set(email.to_string());
    active_model.first_name = Set(first_name);
    active_model.last_name = Set(last_name);
    active_model.is_guest = Set(false);
    active_model.updated_at = Set(Some(chrono::Utc::now().into()));

    // Provider ID'yi ekle
    match provider {
        SocialProvider::Google => active_model.google_id = Set(Some(provider_id.to_string())),
        SocialProvider::Apple => active_model.apple_id = Set(Some(provider_id.to_string())),
    }

    let updated_user = active_model.update(db).await?;
    Ok(updated_user)
}

/// Find or create user for social authentication
pub async fn find_or_create_social_user(
    db: &DatabaseConnection,
    provider: SocialProvider,
    provider_id: &str,
    email: &str,
    first_name: Option<String>,
    last_name: Option<String>,
    guest_user_id: Option<i64>,
) -> Result<UserModel, AuthError> {
    // Eğer guest_user_id varsa, önce guest user'ı gerçek kullanıcıya dönüştürmeyi dene
    if let Some(guest_id) = guest_user_id {
        // Guest user'ın gerçekten var ve guest olduğunu kontrol et
        if let Ok(Some(guest_user)) = User::find_by_id(guest_id).one(db).await {
            if guest_user.is_guest {
                // Guest user'ı sosyal kullanıcıya dönüştür
                match convert_guest_to_social_user(
                    db,
                    guest_id,
                    provider.clone(),
                    provider_id,
                    email,
                    first_name.clone(),
                    last_name.clone(),
                )
                .await
                {
                    Ok(user) => return Ok(user),
                    Err(e) => {
                        eprintln!("Misafir kullanıcı dönüştürme hatası: {}", e);
                        // Hata olursa normal akışa devam et (sepet merge ile)
                    }
                }
            }
        }
    }

    // 1. Provider ID ile kullanıcı ara
    let column = match provider {
        SocialProvider::Google => crate::modules::auth::models::user::Column::GoogleId,
        SocialProvider::Apple => crate::modules::auth::models::user::Column::AppleId,
    };

    let user_by_provider = User::find().filter(column.eq(provider_id)).one(db).await?;

    if let Some(user) = user_by_provider {
        // Misafir kullanıcı sepetini birleştir (eğer varsa)
        if let Some(guest_id) = guest_user_id {
            if let Err(e) = merge_guest_cart_to_user(db, guest_id, user.id).await {
                eprintln!("Misafir sepet birleştirme hatası: {}", e);
            }
        }
        return Ok(user);
    }

    // 2. Email ile kullanıcı ara
    let user_by_email = User::find()
        .filter(crate::modules::auth::models::user::Column::Email.eq(email))
        .one(db)
        .await?;

    if let Some(user) = user_by_email {
        // Kullanıcıya provider ID ekle
        let mut active_model: UserActiveModel = user.into();
        match provider {
            SocialProvider::Google => active_model.google_id = Set(Some(provider_id.to_string())),
            SocialProvider::Apple => active_model.apple_id = Set(Some(provider_id.to_string())),
        }
        active_model.updated_at = Set(Some(chrono::Utc::now().into()));
        let updated_user = active_model.update(db).await?;

        // Misafir kullanıcı sepetini birleştir (eğer varsa)
        if let Some(guest_id) = guest_user_id {
            if let Err(e) = merge_guest_cart_to_user(db, guest_id, updated_user.id).await {
                eprintln!("Misafir sepet birleştirme hatası: {}", e);
            }
        }

        return Ok(updated_user);
    }

    // 3. Yeni kullanıcı oluştur
    let now = chrono::Utc::now();
    let username = email.split('@').next().unwrap_or(provider_id);

    // Benzersiz username bul
    let mut final_username = username.to_string();
    let mut counter = 1;
    while User::find()
        .filter(crate::modules::auth::models::user::Column::Username.eq(&final_username))
        .one(db)
        .await?
        .is_some()
    {
        final_username = format!("{}_{}", username, counter);
        counter += 1;
    }

    let mut active_model = UserActiveModel {
        username: Set(final_username),
        email: Set(email.to_string()),
        password: Set(None), // Sosyal giriş kullanıcıları varsayılan olarak şifresiz
        first_name: Set(first_name),
        last_name: Set(last_name),
        created_at: Set(Some(now.into())),
        updated_at: Set(Some(now.into())),
        ..Default::default()
    };

    match provider {
        SocialProvider::Google => active_model.google_id = Set(Some(provider_id.to_string())),
        SocialProvider::Apple => active_model.apple_id = Set(Some(provider_id.to_string())),
    }

    let new_user = active_model.insert(db).await?;

    // Misafir kullanıcı sepetini birleştir (eğer varsa)
    if let Some(guest_id) = guest_user_id {
        if let Err(e) = merge_guest_cart_to_user(db, guest_id, new_user.id).await {
            eprintln!("Misafir sepet birleştirme hatası: {}", e);
        }
    }

    Ok(new_user)
}

/// Get user by ID
pub async fn get_user_by_id(db: &DatabaseConnection, user_id: i64) -> Result<UserModel, AuthError> {
    User::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or(AuthError::UserNotFound)
}

/// Get user by email
#[allow(dead_code)]
pub async fn get_user_by_email(
    db: &DatabaseConnection,
    email: &str,
) -> Result<UserModel, AuthError> {
    User::find()
        .filter(crate::modules::auth::models::user::Column::Email.eq(email))
        .one(db)
        .await?
        .ok_or(AuthError::UserNotFound)
}

/// Update user profile
#[allow(dead_code)]
pub async fn update_profile(
    db: &DatabaseConnection,
    user_id: i64,
    username: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
) -> Result<UserModel, AuthError> {
    let user = User::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or(AuthError::UserNotFound)?;

    let mut user: UserActiveModel = user.into();

    if let Some(uname) = username {
        user.username = Set(uname);
    }
    if let Some(fname) = first_name {
        user.first_name = Set(Some(fname));
    }
    if let Some(lname) = last_name {
        user.last_name = Set(Some(lname));
    }

    let user = user.update(db).await?;
    Ok(user)
}

/// Change user password
#[allow(dead_code)]
pub async fn change_password(
    db: &DatabaseConnection,
    user_id: i64,
    old_password: &str,
    new_password: &str,
) -> Result<(), AuthError> {
    let user = User::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or(AuthError::UserNotFound)?;

    // Verify old password
    let user_password = user
        .password
        .as_ref()
        .ok_or(AuthError::InvalidCredentials)?;
    let is_valid =
        verify_password(old_password, user_password).map_err(|_| AuthError::InvalidCredentials)?;

    if !is_valid {
        return Err(AuthError::InvalidCredentials);
    }

    // Validate new password
    validate_password_strength(new_password).map_err(|_| AuthError::WeakPassword)?;

    // Hash new password
    let password_hash = hash_password(new_password).map_err(|_| AuthError::WeakPassword)?;

    // Update password
    let mut user: UserActiveModel = user.into();
    user.password = Set(Some(password_hash));
    user.update(db).await?;

    Ok(())
}

/// List all users with pagination and filters (admin only)
pub async fn list_users(
    db: &DatabaseConnection,
    page: u64,
    per_page: u64,
    search: Option<&str>,
    start_date: Option<&str>,
    end_date: Option<&str>,
    is_guest: Option<bool>,
) -> Result<(Vec<UserModel>, u64), AuthError> {
    let offset = (page - 1) * per_page;

    // Base query
    let mut select = User::find();

    // Search filter (username, email, first_name, last_name)
    if let Some(search_term) = search {
        if !search_term.is_empty() {
            let search_pattern = format!("%{}%", search_term.to_lowercase());
            select = select.filter(
                sea_orm::Condition::any()
                    .add(crate::modules::auth::models::user::Column::Username.like(&search_pattern))
                    .add(crate::modules::auth::models::user::Column::Email.like(&search_pattern))
                    .add(
                        crate::modules::auth::models::user::Column::FirstName.like(&search_pattern),
                    )
                    .add(
                        crate::modules::auth::models::user::Column::LastName.like(&search_pattern),
                    ),
            );
        }
    }

    // Date filters
    if let Some(start_date_str) = start_date {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(start_date_str, "%Y-%m-%d") {
            if let Some(datetime) = date.and_hms_opt(0, 0, 0) {
                select = select
                    .filter(crate::modules::auth::models::user::Column::CreatedAt.gte(datetime));
            }
        }
    }

    if let Some(end_date_str) = end_date {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(end_date_str, "%Y-%m-%d") {
            if let Some(datetime) = date.and_hms_opt(23, 59, 59) {
                select = select
                    .filter(crate::modules::auth::models::user::Column::CreatedAt.lte(datetime));
            }
        }
    }

    if let Some(is_guest) = is_guest {
        select = select.filter(crate::modules::auth::models::user::Column::IsGuest.eq(!is_guest));
    }

    // Get total count with filters
    let total = select.clone().count(db).await?;

    // Get users with pagination
    let users = select
        .order_by_desc(crate::modules::auth::models::user::Column::CreatedAt)
        .offset(offset)
        .limit(per_page)
        .all(db)
        .await?;

    Ok((users, total))
}

/// Create new user (admin only)
pub async fn create_user(
    db: &DatabaseConnection,
    username: &str,
    email: &str,
    password: &str,
    first_name: Option<String>,
    last_name: Option<String>,
    phone_number: Option<String>,
    phone_country_code: Option<String>,
    profile: Option<JsonValue>,
) -> Result<UserModel, AuthError> {
    // Check if username or email already exists
    let existing = User::find()
        .filter(
            sea_orm::Condition::any()
                .add(crate::modules::auth::models::user::Column::Username.eq(username))
                .add(crate::modules::auth::models::user::Column::Email.eq(email)),
        )
        .one(db)
        .await?;

    if existing.is_some() {
        return Err(AuthError::UserAlreadyExists);
    }

    // Validate password
    validate_password_strength(password).map_err(|_| AuthError::WeakPassword)?;

    // Hash password
    let hashed_password = hash_password(password).map_err(|_| AuthError::WeakPassword)?;

    // Create user
    let now = chrono::Utc::now();
    let user = UserActiveModel {
        username: Set(username.to_string()),
        email: Set(email.to_string()),
        password: Set(Some(hashed_password)),
        first_name: Set(first_name),
        last_name: Set(last_name),
        phone_number: Set(phone_number),
        phone_country_code: Set(phone_country_code),
        profile: Set(profile),
        created_at: Set(Some(now.into())),
        updated_at: Set(Some(now.into())),
        ..Default::default()
    };

    let user = user.insert(db).await?;
    Ok(user)
}

/// Update user (admin only)
pub async fn update_user(
    db: &DatabaseConnection,
    user_id: i64,
    username: Option<String>,
    email: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
    phone_number: Option<String>,
    phone_country_code: Option<String>,
    profile: Option<JsonValue>,
) -> Result<UserModel, AuthError> {
    let user = User::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or(AuthError::UserNotFound)?;

    let mut active_model: UserActiveModel = user.into();

    if let Some(username) = username {
        // Check if username already exists (excluding current user)
        let existing = User::find()
            .filter(crate::modules::auth::models::user::Column::Username.eq(&username))
            .filter(crate::modules::auth::models::user::Column::Id.ne(user_id))
            .one(db)
            .await?;

        if existing.is_some() {
            return Err(AuthError::UserAlreadyExists);
        }

        active_model.username = Set(username);
    }

    if let Some(email) = email {
        // Check if email already exists (excluding current user)
        let existing = User::find()
            .filter(crate::modules::auth::models::user::Column::Email.eq(&email))
            .filter(crate::modules::auth::models::user::Column::Id.ne(user_id))
            .one(db)
            .await?;

        if existing.is_some() {
            return Err(AuthError::UserAlreadyExists);
        }

        active_model.email = Set(email);
    }

    if let Some(first_name) = first_name {
        active_model.first_name = Set(Some(first_name));
    }

    if let Some(last_name) = last_name {
        active_model.last_name = Set(Some(last_name));
    }

    if let Some(phone_number) = phone_number {
        active_model.phone_number = Set(Some(phone_number));
    }

    if let Some(phone_country_code) = phone_country_code {
        active_model.phone_country_code = Set(Some(phone_country_code));
    }

    if let Some(profile) = profile {
        active_model.profile = Set(Some(profile));
    }

    // Update timestamp
    active_model.updated_at = Set(Some(chrono::Utc::now().into()));

    let user = active_model.update(db).await?;
    Ok(user)
}

/// Update user password (admin only)
pub async fn update_user_password(
    db: &DatabaseConnection,
    user_id: i64,
    new_password: &str,
) -> Result<(), AuthError> {
    // Validate password
    validate_password_strength(new_password).map_err(|_| AuthError::WeakPassword)?;

    // Hash password
    let hashed_password = hash_password(new_password).map_err(|_| AuthError::WeakPassword)?;

    // Update user
    let user = User::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or(AuthError::UserNotFound)?;

    let mut active_model: UserActiveModel = user.into();
    active_model.password = Set(Some(hashed_password));
    active_model.updated_at = Set(Some(chrono::Utc::now().into()));

    active_model.update(db).await?;
    Ok(())
}

/// Delete user (admin only)
pub async fn delete_user(db: &DatabaseConnection, user_id: i64) -> Result<(), AuthError> {
    User::delete_by_id(user_id).exec(db).await?;

    Ok(())
}

/// Update user IP address on login/register
pub async fn update_user_ip(
    db: &DatabaseConnection,
    user_id: i64,
    ip: Option<String>,
) -> Result<(), AuthError> {
    let user = User::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or(AuthError::UserNotFound)?;

    let mut active_model: UserActiveModel = user.into();

    if let Some(ip_addr) = ip {
        // IPv6 adresi mi kontrol et
        if ip_addr.contains(':') {
            active_model.ip_v6 = Set(Some(ip_addr));
        } else {
            active_model.ip = Set(Some(ip_addr));
        }
    }

    active_model.updated_at = Set(Some(chrono::Utc::now().into()));
    active_model.update(db).await?;

    Ok(())
}

/// Request a password reset
pub async fn request_password_reset(
    db: &DatabaseConnection,
    email_or_username: &str,
) -> Result<(UserModel, String), AuthError> {
    // Find user
    let user = User::find()
        .filter(
            sea_orm::Condition::any()
                .add(crate::modules::auth::models::user::Column::Username.eq(email_or_username))
                .add(crate::modules::auth::models::user::Column::Email.eq(email_or_username)),
        )
        .one(db)
        .await?
        .ok_or(AuthError::UserNotFound)?;

    if user.is_guest {
        return Err(AuthError::UserNotFound);
    }

    // Generate token
    let token = uuid::Uuid::new_v4().to_string();
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(10);

    // Save reset token
    let reset = crate::modules::auth::models::password_reset::ActiveModel {
        user_id: Set(user.id),
        token: Set(token.clone()),
        email: Set(user.email.clone()),
        expires_at: Set(expires_at.into()),
        created_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    };
    reset.insert(db).await?;

    Ok((user, token))
}

/// Validate reset token
pub async fn validate_reset_token(
    db: &DatabaseConnection,
    token: &str,
) -> Result<PasswordResetModel, AuthError> {
    let reset = PasswordReset::find()
        .filter(crate::modules::auth::models::password_reset::Column::Token.eq(token))
        .one(db)
        .await?
        .ok_or(AuthError::InvalidCredentials)?;

    if reset.expires_at < chrono::DateTime::<chrono::FixedOffset>::from(chrono::Utc::now()) {
        return Err(AuthError::InvalidCredentials);
    }

    Ok(reset)
}

/// Reset password using token
pub async fn reset_password(
    db: &DatabaseConnection,
    token: &str,
    new_password: &str,
) -> Result<(), AuthError> {
    let reset = validate_reset_token(db, token).await?;

    // Validate new password
    validate_password_strength(new_password).map_err(|_| AuthError::WeakPassword)?;

    // Hash new password
    let password_hash = hash_password(new_password).map_err(|_| AuthError::WeakPassword)?;

    // Update user password
    let user = User::find_by_id(reset.user_id)
        .one(db)
        .await?
        .ok_or(AuthError::UserNotFound)?;

    let mut active_model: UserActiveModel = user.into();
    active_model.password = Set(Some(password_hash));
    active_model.updated_at = Set(Some(chrono::Utc::now().into()));
    active_model.update(db).await?;

    // Delete all reset tokens for this user
    PasswordReset::delete_many()
        .filter(crate::modules::auth::models::password_reset::Column::UserId.eq(reset.user_id))
        .exec(db)
        .await?;

    Ok(())
}
