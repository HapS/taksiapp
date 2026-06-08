use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ============================================
        // 1. USERS & AUTH TABLES
        // ============================================
        
        // Users table
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Users::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Users::Username).string().not_null())
                    .col(ColumnDef::new(Users::FirstName).string())
                    .col(ColumnDef::new(Users::LastName).string())
                    .col(ColumnDef::new(Users::BirthDate).timestamp_with_time_zone())
                    .col(ColumnDef::new(Users::Email).string().not_null())
                    .col(ColumnDef::new(Users::Password).string())
                    .col(ColumnDef::new(Users::Profile).json_binary())
                    .col(ColumnDef::new(Users::GoogleId).string())
                    .col(ColumnDef::new(Users::AppleId).string())
                    .col(ColumnDef::new(Users::IsGuest).boolean().not_null().default(false))
                    .col(ColumnDef::new(Users::GuestSessionId).string())
                    .col(ColumnDef::new(Users::PhoneNumber).string())
                    .col(ColumnDef::new(Users::PhoneCountryCode).string().default("+90"))
                    .col(ColumnDef::new(Users::Ip).string_len(254))
                    .col(ColumnDef::new(Users::IpV6).string_len(254))
                    .col(ColumnDef::new(Users::CreatedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Users::UpdatedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;

        // Sessions table
        manager
            .create_table(
                Table::create()
                    .table(Sessions::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Sessions::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Sessions::UserId).big_integer())
                    .col(ColumnDef::new(Sessions::Data).json().not_null())
                    .col(ColumnDef::new(Sessions::ExpiresAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(Sessions::CreatedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Sessions::UpdatedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;

        manager.create_index(Index::create().name("idx_sessions_user_id").table(Sessions::Table).col(Sessions::UserId).to_owned()).await?;
        manager.create_index(Index::create().name("idx_sessions_expires_at").table(Sessions::Table).col(Sessions::ExpiresAt).to_owned()).await?;

        // ============================================
        // 2. RBAC TABLES (Roles, Permissions)
        // ============================================
        
        // Roles table
        manager
            .create_table(
                Table::create()
                    .table(Roles::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Roles::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Roles::Name).string().not_null().unique_key())
                    .col(ColumnDef::new(Roles::Description).text())
                    .col(ColumnDef::new(Roles::IsSystem).boolean().not_null().default(false))
                    .col(ColumnDef::new(Roles::CreatedAt).timestamp_with_time_zone().default(Expr::current_timestamp()))
                    .col(ColumnDef::new(Roles::UpdatedAt).timestamp_with_time_zone().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        // Permissions table
        manager
            .create_table(
                Table::create()
                    .table(Permissions::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Permissions::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Permissions::Name).string().not_null().unique_key())
                    .col(ColumnDef::new(Permissions::Description).text())
                    .col(ColumnDef::new(Permissions::Module).string().not_null())
                    .col(ColumnDef::new(Permissions::CreatedAt).timestamp_with_time_zone().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        // Role-Permissions junction table
        manager
            .create_table(
                Table::create()
                    .table(RolePermissions::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(RolePermissions::RoleId).big_integer().not_null())
                    .col(ColumnDef::new(RolePermissions::PermissionId).big_integer().not_null())
                    .primary_key(Index::create().col(RolePermissions::RoleId).col(RolePermissions::PermissionId))
                    .foreign_key(ForeignKey::create().from(RolePermissions::Table, RolePermissions::RoleId).to(Roles::Table, Roles::Id).on_delete(ForeignKeyAction::Cascade))
                    .foreign_key(ForeignKey::create().from(RolePermissions::Table, RolePermissions::PermissionId).to(Permissions::Table, Permissions::Id).on_delete(ForeignKeyAction::Cascade))
                    .to_owned(),
            )
            .await?;

        // User-Roles junction table
        manager
            .create_table(
                Table::create()
                    .table(UserRoles::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(UserRoles::UserId).big_integer().not_null())
                    .col(ColumnDef::new(UserRoles::RoleId).big_integer().not_null())
                    .col(ColumnDef::new(UserRoles::CreatedAt).timestamp_with_time_zone().default(Expr::current_timestamp()))
                    .primary_key(Index::create().col(UserRoles::UserId).col(UserRoles::RoleId))
                    .foreign_key(ForeignKey::create().from(UserRoles::Table, UserRoles::UserId).to(Users::Table, Users::Id).on_delete(ForeignKeyAction::Cascade))
                    .foreign_key(ForeignKey::create().from(UserRoles::Table, UserRoles::RoleId).to(Roles::Table, Roles::Id).on_delete(ForeignKeyAction::Cascade))
                    .to_owned(),
            )
            .await?;

        // User-Permissions junction table (direct permissions)
        manager
            .create_table(
                Table::create()
                    .table(UserPermissions::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(UserPermissions::UserId).big_integer().not_null())
                    .col(ColumnDef::new(UserPermissions::PermissionId).big_integer().not_null())
                    .col(ColumnDef::new(UserPermissions::IsGranted).boolean().not_null().default(true))
                    .col(ColumnDef::new(UserPermissions::CreatedAt).timestamp_with_time_zone().default(Expr::current_timestamp()))
                    .primary_key(Index::create().col(UserPermissions::UserId).col(UserPermissions::PermissionId))
                    .foreign_key(ForeignKey::create().from(UserPermissions::Table, UserPermissions::UserId).to(Users::Table, Users::Id).on_delete(ForeignKeyAction::Cascade))
                    .foreign_key(ForeignKey::create().from(UserPermissions::Table, UserPermissions::PermissionId).to(Permissions::Table, Permissions::Id).on_delete(ForeignKeyAction::Cascade))
                    .to_owned(),
            )
            .await?;


        // ============================================
        // 3. CONTENT MANAGEMENT TABLES
        // ============================================
        
        // Contents table (pages, products, etc.)
        manager
            .create_table(
                Table::create()
                    .table(Contents::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Contents::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Contents::Data).json_binary())
                    .col(ColumnDef::new(Contents::Publish).boolean().default(false))
                    .col(ColumnDef::new(Contents::ContentType).text().default("page"))
                    .col(ColumnDef::new(Contents::ParentId).big_integer())
                    .col(ColumnDef::new(Contents::OrderId).integer())
                    .col(ColumnDef::new(Contents::UserId).big_integer())
                    .col(ColumnDef::new(Contents::Gcx).boolean().not_null().default(false))
                    .col(ColumnDef::new(Contents::CreatedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Contents::UpdatedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Contents::DeletedAt).timestamp_with_time_zone())
                    .foreign_key(ForeignKey::create().from(Contents::Table, Contents::ParentId).to(Contents::Table, Contents::Id).on_delete(ForeignKeyAction::SetNull))
                    .foreign_key(ForeignKey::create().from(Contents::Table, Contents::UserId).to(Users::Table, Users::Id).on_delete(ForeignKeyAction::SetNull))
                    .to_owned(),
            )
            .await?;

        // Contents indexes
        manager.create_index(Index::create().name("idx_contents_content_type").table(Contents::Table).col(Contents::ContentType).to_owned()).await?;
        manager.create_index(Index::create().name("idx_contents_publish").table(Contents::Table).col(Contents::Publish).to_owned()).await?;
        manager.create_index(Index::create().name("idx_contents_deleted_at").table(Contents::Table).col(Contents::DeletedAt).to_owned()).await?;
        manager.create_index(Index::create().name("idx_contents_parent_id").table(Contents::Table).col(Contents::ParentId).to_owned()).await?;
        manager.create_index(Index::create().name("idx_contents_order_id").table(Contents::Table).col(Contents::OrderId).to_owned()).await?;
        manager.create_index(Index::create().name("idx_contents_type_order").table(Contents::Table).col(Contents::ContentType).col(Contents::OrderId).to_owned()).await?;

        // Vocabularies table (taxonomy containers)
        manager
            .create_table(
                Table::create()
                    .table(Vocabularies::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Vocabularies::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Vocabularies::Data).json().not_null())
                    .col(ColumnDef::new(Vocabularies::VocabularyType).string().not_null().default("category"))
                    .col(ColumnDef::new(Vocabularies::OrderId).integer())
                    .col(ColumnDef::new(Vocabularies::Gcx).boolean().not_null().default(false))
                    .col(ColumnDef::new(Vocabularies::CreatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .col(ColumnDef::new(Vocabularies::UpdatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        // Terms table (taxonomy items)
        manager
            .create_table(
                Table::create()
                    .table(Terms::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Terms::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Terms::VocabularyId).big_integer().not_null())
                    .col(ColumnDef::new(Terms::Data).json().not_null())
                    .col(ColumnDef::new(Terms::ParentId).big_integer())
                    .col(ColumnDef::new(Terms::Publish).boolean().not_null().default(true))
                    .col(ColumnDef::new(Terms::OrderId).integer())
                    .col(ColumnDef::new(Terms::CreatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .col(ColumnDef::new(Terms::UpdatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .foreign_key(ForeignKey::create().from(Terms::Table, Terms::VocabularyId).to(Vocabularies::Table, Vocabularies::Id).on_delete(ForeignKeyAction::Cascade))
                    .foreign_key(ForeignKey::create().from(Terms::Table, Terms::ParentId).to(Terms::Table, Terms::Id).on_delete(ForeignKeyAction::SetNull))
                    .to_owned(),
            )
            .await?;

        manager.create_index(Index::create().name("idx_terms_order_id").table(Terms::Table).col(Terms::OrderId).to_owned()).await?;

        // Content-Terms junction table
        manager
            .create_table(
                Table::create()
                    .table(ContentTerms::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ContentTerms::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(ContentTerms::ContentId).big_integer().not_null())
                    .col(ColumnDef::new(ContentTerms::TermId).big_integer().not_null())
                    .col(ColumnDef::new(ContentTerms::ContentType).string_len(50).not_null().default("page"))
                    .col(ColumnDef::new(ContentTerms::CreatedAt).timestamp_with_time_zone().default(Expr::current_timestamp()))
                    .foreign_key(ForeignKey::create().from(ContentTerms::Table, ContentTerms::ContentId).to(Contents::Table, Contents::Id).on_delete(ForeignKeyAction::Cascade))
                    .foreign_key(ForeignKey::create().from(ContentTerms::Table, ContentTerms::TermId).to(Terms::Table, Terms::Id).on_delete(ForeignKeyAction::Cascade))
                    .to_owned(),
            )
            .await?;

        // Vocabulary-Categories junction table
        manager
            .create_table(
                Table::create()
                    .table(VocabularyCategories::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(VocabularyCategories::VocabularyId).big_integer().not_null())
                    .col(ColumnDef::new(VocabularyCategories::CategoryTermId).big_integer().not_null())
                    .col(ColumnDef::new(VocabularyCategories::CreatedAt).timestamp_with_time_zone().default(Expr::cust("now()")))
                    .primary_key(Index::create().col(VocabularyCategories::VocabularyId).col(VocabularyCategories::CategoryTermId))
                    .foreign_key(ForeignKey::create().from(VocabularyCategories::Table, VocabularyCategories::VocabularyId).to(Vocabularies::Table, Vocabularies::Id).on_delete(ForeignKeyAction::Cascade))
                    .foreign_key(ForeignKey::create().from(VocabularyCategories::Table, VocabularyCategories::CategoryTermId).to(Terms::Table, Terms::Id).on_delete(ForeignKeyAction::Cascade))
                    .to_owned(),
            )
            .await?;


        // ============================================
        // 4. MEDIA TABLE
        // ============================================
        
        manager
            .create_table(
                Table::create()
                    .table(Media::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Media::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Media::UserId).integer().not_null())
                    .col(ColumnDef::new(Media::FileName).string().not_null())
                    .col(ColumnDef::new(Media::MediaType).string().not_null())
                    .col(ColumnDef::new(Media::MimeType).string().not_null())
                    .col(ColumnDef::new(Media::FilePath).string().not_null())
                    .col(ColumnDef::new(Media::FileSize).big_integer().not_null())
                    .col(ColumnDef::new(Media::Title).string())
                    .col(ColumnDef::new(Media::Description).text())
                    .col(ColumnDef::new(Media::ContentType).string())
                    .col(ColumnDef::new(Media::ContentId).big_integer())
                    .col(ColumnDef::new(Media::CreatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .col(ColumnDef::new(Media::UpdatedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;

        manager.create_index(Index::create().name("idx_media_content").table(Media::Table).col(Media::ContentType).col(Media::ContentId).to_owned()).await?;

        // ============================================
        // 5. ADDRESS TABLES (Countries, Cities, Districts)
        // ============================================
        
        // Countries table
        manager
            .create_table(
                Table::create()
                    .table(Countries::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Countries::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Countries::Name).string().not_null())
                    .col(ColumnDef::new(Countries::Code).string())
                    .col(ColumnDef::new(Countries::PhoneCode).string())
                    .to_owned(),
            )
            .await?;

        // Cities table
        manager
            .create_table(
                Table::create()
                    .table(Cities::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Cities::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Cities::CountryId).big_integer().not_null())
                    .col(ColumnDef::new(Cities::Name).string().not_null())
                    .foreign_key(ForeignKey::create().from(Cities::Table, Cities::CountryId).to(Countries::Table, Countries::Id).on_delete(ForeignKeyAction::Cascade))
                    .to_owned(),
            )
            .await?;

        // Districts table
        manager
            .create_table(
                Table::create()
                    .table(Districts::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Districts::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Districts::CityId).big_integer().not_null())
                    .col(ColumnDef::new(Districts::Name).string().not_null())
                    .foreign_key(ForeignKey::create().from(Districts::Table, Districts::CityId).to(Cities::Table, Cities::Id).on_delete(ForeignKeyAction::Cascade))
                    .to_owned(),
            )
            .await?;

        // Addresses table
        manager
            .create_table(
                Table::create()
                    .table(Addresses::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Addresses::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Addresses::UserId).big_integer().not_null())
                    .col(ColumnDef::new(Addresses::Title).string().not_null())
                    .col(ColumnDef::new(Addresses::CountryId).big_integer().not_null())
                    .col(ColumnDef::new(Addresses::CityId).big_integer().not_null())
                    .col(ColumnDef::new(Addresses::DistrictId).big_integer().not_null())
                    .col(ColumnDef::new(Addresses::AddressLine).text().not_null())
                    .col(ColumnDef::new(Addresses::IsDefault).boolean().not_null().default(false))
                    .col(ColumnDef::new(Addresses::PhoneCountryCode).string().not_null().default("+90"))
                    .col(ColumnDef::new(Addresses::PhoneNumber).string().not_null().default(""))
                    .col(ColumnDef::new(Addresses::CreatedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Addresses::UpdatedAt).timestamp_with_time_zone())
                    .foreign_key(ForeignKey::create().from(Addresses::Table, Addresses::UserId).to(Users::Table, Users::Id).on_delete(ForeignKeyAction::Cascade))
                    .foreign_key(ForeignKey::create().from(Addresses::Table, Addresses::CountryId).to(Countries::Table, Countries::Id))
                    .foreign_key(ForeignKey::create().from(Addresses::Table, Addresses::CityId).to(Cities::Table, Cities::Id))
                    .foreign_key(ForeignKey::create().from(Addresses::Table, Addresses::DistrictId).to(Districts::Table, Districts::Id))
                    .to_owned(),
            )
            .await?;

        // Corporate Infos table
        manager
            .create_table(
                Table::create()
                    .table(CorporateInfos::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(CorporateInfos::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(CorporateInfos::UserId).big_integer().not_null())
                    .col(ColumnDef::new(CorporateInfos::Title).string().not_null())
                    .col(ColumnDef::new(CorporateInfos::TaxOffice).string().not_null())
                    .col(ColumnDef::new(CorporateInfos::TaxNumber).string().not_null())
                    .col(ColumnDef::new(CorporateInfos::CompanyName).string().not_null())
                    .col(ColumnDef::new(CorporateInfos::CreatedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(CorporateInfos::UpdatedAt).timestamp_with_time_zone())
                    .foreign_key(ForeignKey::create().from(CorporateInfos::Table, CorporateInfos::UserId).to(Users::Table, Users::Id).on_delete(ForeignKeyAction::Cascade))
                    .to_owned(),
            )
            .await?;


        // ============================================
        // 6. E-COMMERCE TABLES (Carts, Cart Items)
        // ============================================
        
        // Carts table
        manager
            .create_table(
                Table::create()
                    .table(Carts::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Carts::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Carts::UserId).big_integer().not_null())
                    .col(ColumnDef::new(Carts::Status).string().not_null().default("open_cart"))
                    .col(ColumnDef::new(Carts::AddressId).big_integer())
                    .col(ColumnDef::new(Carts::InvoiceId).big_integer())
                    .col(ColumnDef::new(Carts::AddressLine).text())
                    .col(ColumnDef::new(Carts::InvoiceAddressLine).text())
                    .col(ColumnDef::new(Carts::PaymentMethod).string())
                    .col(ColumnDef::new(Carts::OrderId).string())
                    .col(ColumnDef::new(Carts::PaymentUrl).string())
                    .col(ColumnDef::new(Carts::CallbackData).json_binary())
                    .col(ColumnDef::new(Carts::Notes).text())
                    .col(ColumnDef::new(Carts::AdminNotes).text())
                    .col(ColumnDef::new(Carts::TotalAmount).decimal())
                    .col(ColumnDef::new(Carts::Currency).string().default("TRY"))
                    .col(ColumnDef::new(Carts::CargoCompany).string())
                    .col(ColumnDef::new(Carts::CargoTrackingNo).string())
                    .col(ColumnDef::new(Carts::OrderDate).timestamp_with_time_zone())
                    .col(ColumnDef::new(Carts::CompletedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Carts::CreatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .col(ColumnDef::new(Carts::UpdatedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;

        manager.create_index(Index::create().name("idx_carts_user_id").table(Carts::Table).col(Carts::UserId).to_owned()).await?;
        manager.create_index(Index::create().name("idx_carts_status_user_id").table(Carts::Table).col(Carts::Status).col(Carts::UserId).to_owned()).await?;
        manager.create_index(Index::create().name("idx_carts_completed_at").table(Carts::Table).col(Carts::CompletedAt).to_owned()).await?;

        // Cart Items table
        manager
            .create_table(
                Table::create()
                    .table(CartItems::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(CartItems::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(CartItems::CartId).big_integer().not_null())
                    .col(ColumnDef::new(CartItems::ProductId).big_integer().not_null())
                    .col(ColumnDef::new(CartItems::VariantKey).string())
                    .col(ColumnDef::new(CartItems::VariantDisplay).string())
                    .col(ColumnDef::new(CartItems::Quantity).integer().not_null().default(1))
                    .col(ColumnDef::new(CartItems::ProductMetaData).json())
                    .col(ColumnDef::new(CartItems::Currency).string().default("TRY"))
                    .col(ColumnDef::new(CartItems::OriginalPrice).decimal())
                    .col(ColumnDef::new(CartItems::CreatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .col(ColumnDef::new(CartItems::UpdatedAt).timestamp_with_time_zone())
                    .foreign_key(ForeignKey::create().from(CartItems::Table, CartItems::CartId).to(Carts::Table, Carts::Id).on_delete(ForeignKeyAction::Cascade))
                    .to_owned(),
            )
            .await?;

        manager.create_index(Index::create().name("idx_cart_items_cart_id").table(CartItems::Table).col(CartItems::CartId).to_owned()).await?;

        // ============================================
        // 7. TIMELINE EVENTS TABLE
        // ============================================
        
        manager
            .create_table(
                Table::create()
                    .table(TimelineEvents::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(TimelineEvents::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(TimelineEvents::ModuleType).string_len(50).not_null())
                    .col(ColumnDef::new(TimelineEvents::ContentType).string_len(50).not_null())
                    .col(ColumnDef::new(TimelineEvents::ContentId).big_integer().not_null())
                    .col(ColumnDef::new(TimelineEvents::EventType).string_len(100).not_null())
                    .col(ColumnDef::new(TimelineEvents::Title).json().not_null())
                    .col(ColumnDef::new(TimelineEvents::Description).json())
                    .col(ColumnDef::new(TimelineEvents::Icon).string_len(50))
                    .col(ColumnDef::new(TimelineEvents::Color).string_len(20).default("primary"))
                    .col(ColumnDef::new(TimelineEvents::UserId).big_integer())
                    .col(ColumnDef::new(TimelineEvents::AdminUserId).big_integer())
                    .col(ColumnDef::new(TimelineEvents::Metadata).json())
                    .col(ColumnDef::new(TimelineEvents::IsPublic).boolean().not_null().default(true))
                    .col(ColumnDef::new(TimelineEvents::IsAdminOnly).boolean().not_null().default(false))
                    .col(ColumnDef::new(TimelineEvents::CreatedAt).timestamp_with_time_zone().default(Expr::current_timestamp()))
                    .col(ColumnDef::new(TimelineEvents::UpdatedAt).timestamp_with_time_zone().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;


        // ============================================
        // 8. SETTINGS TABLE
        // ============================================
        
        manager
            .create_table(
                Table::create()
                    .table(Settings::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Settings::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Settings::Data).json_binary())
                    .col(ColumnDef::new(Settings::CreatedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Settings::UpdatedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;

        // ============================================
        // 9. MAIL QUEUE TABLE
        // ============================================
        
        manager
            .create_table(
                Table::create()
                    .table(MailQueue::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(MailQueue::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(MailQueue::ToEmail).string().not_null())
                    .col(ColumnDef::new(MailQueue::ToName).string())
                    .col(ColumnDef::new(MailQueue::Subject).string().not_null())
                    .col(ColumnDef::new(MailQueue::Body).text().not_null())
                    .col(ColumnDef::new(MailQueue::TemplateName).string())
                    .col(ColumnDef::new(MailQueue::Variables).json())
                    .col(ColumnDef::new(MailQueue::Language).string().default("tr"))
                    .col(ColumnDef::new(MailQueue::Status).string().default("pending"))
                    .col(ColumnDef::new(MailQueue::Attempts).integer().default(0))
                    .col(ColumnDef::new(MailQueue::MaxAttempts).integer().default(3))
                    .col(ColumnDef::new(MailQueue::ErrorMessage).text())
                    .col(ColumnDef::new(MailQueue::ScheduledAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(MailQueue::SentAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(MailQueue::CreatedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(MailQueue::UpdatedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;

        manager.create_index(Index::create().name("idx_mail_queue_status").table(MailQueue::Table).col(MailQueue::Status).to_owned()).await?;
        manager.create_index(Index::create().name("idx_mail_queue_scheduled").table(MailQueue::Table).col(MailQueue::ScheduledAt).to_owned()).await?;
        manager.create_index(Index::create().name("idx_mail_queue_template_name").table(MailQueue::Table).col(MailQueue::TemplateName).to_owned()).await?;

        // ============================================
        // 10. EXCHANGE RATES TABLE
        // ============================================
        
        manager
            .create_table(
                Table::create()
                    .table(ExchangeRates::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ExchangeRates::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(ExchangeRates::UsdTry).decimal())
                    .col(ColumnDef::new(ExchangeRates::EurTry).decimal())
                    .col(ColumnDef::new(ExchangeRates::GbpTry).decimal())
                    .col(ColumnDef::new(ExchangeRates::ChfTry).decimal())
                    .col(ColumnDef::new(ExchangeRates::AudTry).decimal())
                    .col(ColumnDef::new(ExchangeRates::CadTry).decimal())
                    .col(ColumnDef::new(ExchangeRates::EurUsd).decimal())
                    .col(ColumnDef::new(ExchangeRates::Source).string_len(50))
                    .col(ColumnDef::new(ExchangeRates::CreatedAt).timestamp_with_time_zone().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        manager.create_index(Index::create().name("idx_exchange_rates_created_at").table(ExchangeRates::Table).col(ExchangeRates::CreatedAt).to_owned()).await?;

        // ============================================
        // 11. HOMEPAGE TABLE
        // ============================================
        
        manager
            .create_table(
                Table::create()
                    .table(Homepage::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Homepage::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Homepage::Data).json_binary().not_null().default("[]"))
                    .col(ColumnDef::new(Homepage::CreatedAt).timestamp_with_time_zone().default(Expr::current_timestamp()))
                    .col(ColumnDef::new(Homepage::UpdatedAt).timestamp_with_time_zone().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop tables in reverse order (respecting foreign key dependencies)
        manager.drop_table(Table::drop().table(Homepage::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(ExchangeRates::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(MailQueue::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(Settings::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(TimelineEvents::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(CartItems::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(Carts::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(CorporateInfos::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(Addresses::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(Districts::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(Cities::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(Countries::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(Media::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(VocabularyCategories::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(ContentTerms::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(Terms::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(Vocabularies::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(Contents::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(UserPermissions::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(UserRoles::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(RolePermissions::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(Permissions::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(Roles::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(Sessions::Table).if_exists().to_owned()).await?;
        manager.drop_table(Table::drop().table(Users::Table).if_exists().to_owned()).await?;
        Ok(())
    }
}


// ============================================
// TABLE DEFINITIONS (Iden enums)
// ============================================

#[derive(Iden)]
enum Users {
    Table,
    Id,
    Username,
    FirstName,
    LastName,
    BirthDate,
    Email,
    Password,
    Profile,
    GoogleId,
    AppleId,
    IsGuest,
    GuestSessionId,
    PhoneNumber,
    PhoneCountryCode,
    Ip,
    IpV6,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Sessions {
    Table,
    Id,
    UserId,
    Data,
    ExpiresAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Roles {
    Table,
    Id,
    Name,
    Description,
    IsSystem,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Permissions {
    Table,
    Id,
    Name,
    Description,
    Module,
    CreatedAt,
}

#[derive(Iden)]
enum RolePermissions {
    Table,
    RoleId,
    PermissionId,
}

#[derive(Iden)]
enum UserRoles {
    Table,
    UserId,
    RoleId,
    CreatedAt,
}

#[derive(Iden)]
enum UserPermissions {
    Table,
    UserId,
    PermissionId,
    IsGranted,
    CreatedAt,
}

#[derive(Iden)]
enum Contents {
    Table,
    Id,
    Data,
    Publish,
    ContentType,
    ParentId,
    OrderId,
    UserId,
    Gcx,
    CreatedAt,
    UpdatedAt,
    DeletedAt,
}

#[derive(Iden)]
enum Vocabularies {
    Table,
    Id,
    Data,
    VocabularyType,
    OrderId,
    Gcx,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Terms {
    Table,
    Id,
    VocabularyId,
    Data,
    ParentId,
    Publish,
    OrderId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum ContentTerms {
    Table,
    Id,
    ContentId,
    TermId,
    ContentType,
    CreatedAt,
}

#[derive(Iden)]
enum VocabularyCategories {
    Table,
    VocabularyId,
    CategoryTermId,
    CreatedAt,
}

#[derive(Iden)]
enum Media {
    Table,
    Id,
    UserId,
    FileName,
    MediaType,
    MimeType,
    FilePath,
    FileSize,
    Title,
    Description,
    ContentType,
    ContentId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Countries {
    Table,
    Id,
    Name,
    Code,
    PhoneCode,
}

#[derive(Iden)]
enum Cities {
    Table,
    Id,
    CountryId,
    Name,
}

#[derive(Iden)]
enum Districts {
    Table,
    Id,
    CityId,
    Name,
}

#[derive(Iden)]
enum Addresses {
    Table,
    Id,
    UserId,
    Title,
    CountryId,
    CityId,
    DistrictId,
    AddressLine,
    IsDefault,
    PhoneCountryCode,
    PhoneNumber,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum CorporateInfos {
    Table,
    Id,
    UserId,
    Title,
    TaxOffice,
    TaxNumber,
    CompanyName,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Carts {
    Table,
    Id,
    UserId,
    Status,
    AddressId,
    InvoiceId,
    AddressLine,
    InvoiceAddressLine,
    PaymentMethod,
    OrderId,
    PaymentUrl,
    CallbackData,
    Notes,
    AdminNotes,
    TotalAmount,
    Currency,
    CargoCompany,
    CargoTrackingNo,
    OrderDate,
    CompletedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum CartItems {
    Table,
    Id,
    CartId,
    ProductId,
    VariantKey,
    VariantDisplay,
    Quantity,
    ProductMetaData,
    Currency,
    OriginalPrice,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum TimelineEvents {
    Table,
    Id,
    ModuleType,
    ContentType,
    ContentId,
    EventType,
    Title,
    Description,
    Icon,
    Color,
    UserId,
    AdminUserId,
    Metadata,
    IsPublic,
    IsAdminOnly,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Settings {
    Table,
    Id,
    Data,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum MailQueue {
    Table,
    Id,
    ToEmail,
    ToName,
    Subject,
    Body,
    TemplateName,
    Variables,
    Language,
    Status,
    Attempts,
    MaxAttempts,
    ErrorMessage,
    ScheduledAt,
    SentAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum ExchangeRates {
    Table,
    Id,
    UsdTry,
    EurTry,
    GbpTry,
    ChfTry,
    AudTry,
    CadTry,
    EurUsd,
    Source,
    CreatedAt,
}

#[derive(Iden)]
enum Homepage {
    Table,
    Id,
    Data,
    CreatedAt,
    UpdatedAt,
}
