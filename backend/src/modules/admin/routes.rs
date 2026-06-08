use super::controllers::{api, web};
use crate::app_state::AppState;
use axum::{
    routing::{delete, get, post, put},
    Router,
};

// Admin app URL patterns - Axum router
pub fn routes() -> Router<AppState> {
    // Admin routes with middleware
    let admin_routes = Router::new()
        // Web: Admin Dashboard & Home
        .route("/admin", get(web::dashboard))
        .route(
            "/admin/home-page-builder",
            get(web::homepage::home_page_builder),
        )
        // Settings routes - modular approach
        .route("/admin/settings", get(web::settings::settings_index))
        //.route("/admin/settings/update", get(web::settings::update::system_update).post(web::settings::update::system_update_zip_file))
        .route(
            "/admin/settings/general",
            get(web::settings::general::general_settings)
                .post(web::settings::general::update_general_settings),
        )
        .route(
            "/admin/settings/appearance",
            get(web::settings::appearance::appearance_settings)
                .post(web::settings::appearance::update_appearance_settings),
        )
        .route(
            "/admin/settings/seo",
            get(web::settings::seo::seo_settings).post(web::settings::seo::update_seo_settings),
        )
        .route(
            "/admin/settings/social",
            get(web::settings::social::social_settings)
                .post(web::settings::social::update_social_settings),
        )
        .route(
            "/admin/settings/mail",
            get(web::settings::mail::mail_settings).post(web::settings::mail::update_mail_settings),
        )
        .route(
            "/admin/settings/notifications",
            get(web::settings::notifications::notification_settings)
                .post(web::settings::notifications::update_notification_settings),
        )
        .route(
            "/admin/settings/payment",
            get(web::settings::payment::payment_settings)
                .post(web::settings::payment::update_payment_settings),
        )
        .route(
            "/admin/settings/payment-methods",
            get(web::settings::payment_methods::payment_methods_settings)
                .post(web::settings::payment_methods::update_payment_methods_settings),
        )
        .route(
            "/admin/settings/security",
            get(web::settings::security::security_settings)
                .post(web::settings::security::update_security_settings),
        )
        .route(
            "/admin/settings/advanced",
            get(web::settings::advanced::advanced_settings)
                .post(web::settings::advanced::update_advanced_settings),
        )
        .route(
            "/admin/settings/kargo-sirketleri",
            get(web::settings::kargo::kargo_sirketleri_list)
                .post(web::settings::kargo::cargo_extra_settings_post),
        )
        .route(
            "/admin/settings/kargo-sirketi/{kargo_sirketi_id}",
            get(web::settings::kargo::kargo_sirketi_settings)
                .post(web::settings::kargo::kargo_sirketi_settings_post),
        )
        // Web: Taxonomy Management HTML Views
        .route("/admin/taxonomy", get(web::taxonomy::taxonomy_manager))
        .route(
            "/admin/taxonomy/vocabularies",
            get(web::taxonomy::taxonomy_manager),
        )
        .route(
            "/admin/taxonomy/{vocabulary_type}",
            get(web::taxonomy::vocabulary_list_by_type),
        )
        .route(
            "/admin/taxonomy/vocabularies/{vocabulary_id}/terms",
            get(web::taxonomy::term_manager),
        )
        .route(
            "/admin/taxonomy/vocabularies/{vocabulary_id}/terms/new",
            get(web::taxonomy::term_add),
        )
        .route(
            "/admin/taxonomy/vocabularies/{vocabulary_id}/terms/{term_id}",
            get(web::taxonomy::term_edit),
        )
        // Admin Content HTML Views
        .route("/admin/contents", get(web::content::admin_content_list))
        .route(
            "/admin/contents/products-import",
            get(web::products_import::products_import),
        )
        .route(
            "/admin/contents/new",
            get(web::content::admin_content_create),
        )
        .route(
            "/admin/contents/{content_type}/{id}",
            get(web::content::admin_content_detail),
        )
        .route(
            "/admin/api/build-content-absolute-url/search",
            get(api::build_content_absolute_url::search_absolute_url),
        )
        // Admin Orders HTML Views
        .route("/admin/orders", get(web::orders::orders_list))
        .route("/admin/orders/{id}", get(web::orders::order_edit))
        // Admin Campaigns HTML Views
        .route("/admin/campaigns", get(web::campaign::campaign_list))
        .route(
            "/admin/campaigns/new",
            get(web::campaign::campaign_create),
        )
        .route(
            "/admin/campaigns/{id}",
            get(web::campaign::campaign_edit),
        )
        .route(
            "/admin/campaigns/{id}/coupons",
            get(web::campaign::coupon_list),
        )
        // Admin Returns HTML Views
        .route("/admin/returns", get(web::returns::returns_list))
        // Admin Mailer HTML Views
        .route("/admin/mailer", get(web::mailer::admin_mailer_list))
        // API: Vocabulary Management
        .route(
            "/admin/api/vocabularies",
            get(api::vocabulary::list).post(api::vocabulary::create),
        )
        .route(
            "/admin/api/vocabularies/type/{vocabulary_type}",
            get(api::vocabulary::list_by_type),
        )
        .route(
            "/admin/api/vocabularies/{id}",
            get(api::vocabulary::get_by_id)
                .put(api::vocabulary::update)
                .delete(api::vocabulary::delete),
        )
        .route(
            "/admin/api/vocabularies/update-order",
            post(api::vocabulary::update_order),
        )
        // API: Term Management
        .route(
            "/admin/api/terms",
            get(api::term::list).post(api::term::create),
        )
        .route(
            "/admin/api/terms/update-order",
            post(api::term::update_order),
        )
        .route(
            "/admin/api/terms/{id}",
            get(api::term::get_by_id)
                .put(api::term::update)
                .delete(api::term::delete),
        )
        .route(
            "/admin/api/terms/{id}/toggle-publish",
            post(api::term::toggle_publish),
        )
        .route(
            "/admin/api/vocabularies/{vocabulary_id}/terms",
            get(api::term::get_by_vocabulary).post(api::term::create),
        )
        // Bulk Import API Routes
        .route(
            "/admin/api/products/bulk-import",
            post(api::bulk_import::bulk_import_products),
        )
        .route(
            "/admin/api/products/bulk-import/test",
            get(api::bulk_import::test_bulk_import_endpoint),
        )
        // Admin Content API Routes
        .route(
            "/admin/api/contents",
            get(api::content::admin_api_list_contents).post(api::content::admin_api_create_content),
        )
        .route(
            "/admin/api/contents/{id}",
            get(api::content::admin_api_get_content)
                .put(api::content::admin_api_update_content)
                .delete(api::content::admin_api_delete_content),
        )
        .route(
            "/admin/api/contents/update-order",
            post(api::content::admin_api_update_content_order),
        )
        .route(
            "/admin/api/contents/{id}/toggle-publish",
            post(api::content::admin_api_toggle_publish_content),
        )
        // Language Management API Routes
        .route(
            "/admin/api/languages",
            get(api::content::admin_api_list_languages),
        )
        // Template Management API Routes
        .route(
            "/admin/api/templates",
            get(api::content::admin_api_list_templates),
        )
        .route(
            "/admin/api/section-templates",
            get(api::content::admin_api_list_section_templates),
        )
        // Taxonomy Integration API Routes
        .route(
            "/admin/api/content-types/{content_type}/terms",
            get(api::content::admin_api_get_terms_by_content_type),
        )
        .route(
            "/admin/api/categories/attributes",
            post(api::content::get_categories_attributes),
        )
        .route(
            "/admin/api/vocabularies/{vocabulary_id}/categories",
            get(api::content::get_vocabulary_categories)
                .post(api::content::update_vocabulary_categories),
        )
        // Homepage API Routes - Tek endpoint, tüm sections JSON'da
        .route(
            "/admin/api/homepage",
            get(web::homepage::api_get_homepage_sections)
                .put(web::homepage::api_update_homepage_sections),
        )
        // Homepage Vocabulary API Route (homepage builder için)
        .route(
            "/admin/api/homepage/vocabularies",
            get(web::homepage::api_get_vocabularies),
        )
        // Homepage Render API (önizleme için)
        .route(
            "/api/homepage/render",
            get(web::homepage::api_render_homepage),
        )
        // Admin Orders API Routes
        .route("/admin/api/orders", get(api::orders::get_orders))
        .route("/admin/api/orders/{id}", get(api::orders::get_order))
        .route(
            "/admin/api/orders/{id}/status",
            put(api::orders::update_order_status),
        )
        //order larda gelecek kargo şirketleri listesi selectbox için
        .route(
            "/admin/api/orders/cargo_companies",
            get(api::orders::kargo_sirketleri_list),
        )
        // İptal talebi yönetimi
        .route(
            "/admin/api/orders/{cart_id}/items/{item_id}/cancel/accept",
            put(api::orders::accept_cancel_request),
        )
        .route(
            "/admin/api/orders/{cart_id}/items/{item_id}/cancel/reject",
            put(api::orders::reject_cancel_request),
        )
        // Admin İade Talebi Yönetimi API Routes
        .route(
            "/admin/api/returns",
            get(api::return_admin::list_return_requests),
        )
        .route(
            "/admin/api/returns/{return_id}",
            get(api::return_admin::get_return_request),
        )
        .route(
            "/admin/api/returns/{return_id}/approve",
            put(api::return_admin::approve_return_request),
        )
        .route(
            "/admin/api/returns/{return_id}/reject",
            put(api::return_admin::reject_return_request),
        )
        .route(
            "/admin/api/returns/{return_id}/received",
            put(api::return_admin::mark_return_received),
        )
        .route(
            "/admin/api/returns/{return_id}/complete",
            put(api::return_admin::complete_return_request),
        )
        .route(
            "/admin/api/returns/{return_id}/notes",
            put(api::return_admin::update_admin_notes),
        )
        // B2B Credit Management HTML Views
        .route(
            "/admin/b2b/credit/transactions",
            get(web::credit::credit_transactions_page),
        )
        .route(
            "/admin/b2b/credit/payment/{company_id}",
            get(web::credit::credit_payment_page),
        )
        .route(
            "/admin/b2b/credit/adjustment/{company_id}",
            get(web::credit::credit_adjustment_page),
        )
        // B2B Credit Management API Routes
        .route(
            "/admin/api/credit/transactions",
            get(api::credit_api::get_credit_transactions),
        )
        .route(
            "/admin/api/credit/summary",
            get(api::credit_api::get_company_credit_summary),
        )
        .route(
            "/admin/api/credit/payment",
            post(api::credit_api::create_payment),
        )
        .route(
            "/admin/api/credit/adjustment",
            post(api::credit_api::create_adjustment),
        )
        // B2B Commission Management HTML Views
        .route(
            "/admin/b2b/commission/transactions",
            get(web::commission::commission_transactions_page),
        )
        .route(
            "/admin/b2b/commission/payment/{representative_id}",
            get(web::commission::commission_payment_page),
        )
        .route(
            "/admin/b2b/commission/adjustment/{representative_id}",
            get(web::commission::commission_adjustment_page),
        )
        .route(
            "/admin/b2b/representatives",
            get(web::commission::representatives_list_page),
        )
        // B2B Commission Management API Routes
        .route(
            "/admin/api/commission/transactions",
            get(api::commission_api::get_commission_transactions),
        )
        .route(
            "/admin/api/commission/summary",
            get(api::commission_api::get_representative_commission_summary),
        )
        .route(
            "/admin/api/commission/payment",
            post(api::commission_api::create_commission_payment),
        )
        .route(
            "/admin/api/commission/adjustment",
            post(api::commission_api::create_commission_adjustment),
        )
        .route(
            "/admin/api/commission/representatives",
            get(api::commission_api::get_representatives),
        )
        // Refund Management API Routes
        .route(
            "/admin/api/refund/bulk-b2b-credit",
            post(api::refund_api::bulk_refund_to_b2b_credit),
        )
        .route(
            "/admin/api/refund/bulk-bank",
            post(api::refund_api::bulk_mark_bank_refunded),
        )
        // User Credit Management HTML Views
        .route("/admin/credits", get(web::credit::user_credits_page))
        // User Management HTML Views
        .route("/admin/users", get(web::users::user_list))
        .route("/admin/users/new", get(web::users::user_create))
        .route("/admin/users/{id}", get(web::users::user_edit))
        // User Management API Routes
        .route(
            "/admin/api/users",
            get(api::users::list_users).post(api::users::create_user),
        )
        .route(
            "/admin/api/users/{id}",
            get(api::users::get_user)
                .put(api::users::update_user)
                .delete(api::users::delete_user),
        )
        .route(
            "/admin/api/users/{id}/password",
            put(api::users::update_password),
        )
        // Role & Permission Management API Routes
        .route("/admin/api/roles", get(api::role::list_roles))
        .route(
            "/admin/api/users/{user_id}/roles",
            get(api::role::get_user_roles).post(api::role::assign_role),
        )
        .route(
            "/admin/api/users/{user_id}/roles/{role_id}",
            axum::routing::delete(api::role::remove_role),
        )
        .route(
            "/admin/api/users/{user_id}/permissions",
            get(api::role::get_user_permissions).post(api::role::grant_permission),
        )
        .route(
            "/admin/api/users/{user_id}/permissions/{permission_id}",
            axum::routing::delete(api::role::revoke_permission)
                .put(api::role::remove_permission_override),
        )
        .route(
            "/admin/api/users/{user_id}/permission-overrides",
            get(api::role::get_user_permission_overrides),
        )
        .route(
            "/admin/api/permissions/by-module",
            get(api::role::list_permissions_by_module),
        )
        // Test Mail API Routes
        .route(
            "/admin/api/test-mail",
            post(api::mailer::admin_api_send_test_mail),
        )
        // Simple Mail API Route (flexible data-based mail sending)
        .route(
            "/admin/api/send-simple-mail",
            post(api::mailer::admin_api_send_simple_mail),
        )
        // Simple Test Mail API Route (no auth)
        .route(
            "/admin/api/test-mail-simple",
            get(api::mailer::admin_api_test_mail_simple),
        )
        // Mail Queue Management API Routes
        .route(
            "/admin/api/mailer/queue",
            get(api::mailer::admin_api_list_mail_queue),
        )
        .route(
            "/admin/api/mailer/stats",
            get(api::mailer::admin_api_get_mail_queue_stats),
        )
        .route(
            "/admin/api/mailer/process",
            post(api::mailer::admin_api_process_mail_queue),
        )
        .route(
            "/admin/api/mailer/retry/{id}",
            post(api::mailer::admin_api_retry_mail),
        )
        .route(
            "/admin/api/mailer/delete/{id}",
            delete(api::mailer::admin_api_delete_mail),
        )
        // Admin Locales HTML Views
        .route("/admin/locales", get(web::locales::locales_manager))
        // Admin Locales API Routes
        .route(
            "/admin/api/locales",
            get(api::locales::list).post(api::locales::update),
        )
        .route("/admin/api/locales/delete", post(api::locales::delete));

    admin_routes
}
