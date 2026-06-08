pub use sea_orm_migration::prelude::*;

mod m20260112_000001_initial_schema;
mod m20260115_165500_add_azn_jpy_to_exchange_rates;
mod m20260119_120000_unify_addresses;
mod m20260121_090000_create_password_resets_table;
mod m20260121_154900_create_form_submissions_table;
mod m20260130_232437_create_bookmarks_table;
mod m20260201_000001_add_variant_key_to_bookmarks;
mod m20260202_000001_change_bookmarks_price_to_text;
mod m20260204_082600_vocab_hide_lock_field;
mod m20260204_083516_term_hide_lock_field;
mod m20260205_064036_kargo_sirketleri;
mod m20260208_000001_change_cargo_company_to_integer;
mod m20260215_093848_user_type_b2b_b2c;
mod m20260220_000001_create_companies_table;
mod m20260220_000002_create_company_users_table;
mod m20260220_000003_create_company_representatives_table;
mod m20260220_000004_add_logo_to_companies;
mod m20260220_000005_add_commission_rate_to_companies;
mod m20260222_000001_add_cart_type_to_carts;
mod m20260224_000001_add_cargo_price_to_carts;
mod m20260302_000001_add_status_to_cart_items;
mod m20260303_000001_create_b2b_credit_transactions;
mod m20260303_000002_create_representative_commission_transactions;
mod m20260303_000003_remove_representative_columns_from_companies;
mod m20260305_000001_add_refund_fields_to_cart_items;
mod m20260306_000001_create_return_requests_table;
mod m20260311_000001_add_refund_currency_to_cart_items;
mod m20260314_000001_add_currency_to_companies;
mod m20260416_143557_comment;
mod m20260419_215522_add_payment_due_days_to_carts;
mod m20260426_000001_add_discount_percentage_to_cart_items;
mod m20260428_000001_create_campaigns_table;
mod m20260428_000002_create_coupons_table;
mod m20260428_000003_create_cart_discounts_table;
mod m20260428_000004_create_campaign_usages_table;
mod m20260501_130800_add_target_cart_type_to_campaigns;
mod m20260523_000001_create_ride_tables;
mod m20260526_000001_add_driver_user_id_unique;
mod m20260526_000002_add_ride_indexes;
mod m20260527_000001_create_ride_fare_configs;
mod m20260604_000001_create_locations;
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260112_000001_initial_schema::Migration),
            Box::new(m20260115_165500_add_azn_jpy_to_exchange_rates::Migration),
            Box::new(m20260119_120000_unify_addresses::Migration),
            Box::new(m20260121_090000_create_password_resets_table::Migration),
            Box::new(m20260121_154900_create_form_submissions_table::Migration),
            Box::new(m20260130_232437_create_bookmarks_table::Migration),
            Box::new(m20260201_000001_add_variant_key_to_bookmarks::Migration),
            Box::new(m20260202_000001_change_bookmarks_price_to_text::Migration),
            Box::new(m20260204_082600_vocab_hide_lock_field::Migration),
            Box::new(m20260204_083516_term_hide_lock_field::Migration),
            Box::new(m20260205_064036_kargo_sirketleri::Migration),
            Box::new(m20260208_000001_change_cargo_company_to_integer::Migration),
            Box::new(m20260215_093848_user_type_b2b_b2c::Migration),
            Box::new(m20260220_000001_create_companies_table::Migration),
            Box::new(m20260220_000002_create_company_users_table::Migration),
            Box::new(m20260220_000003_create_company_representatives_table::Migration),
            Box::new(m20260220_000004_add_logo_to_companies::Migration),
            Box::new(m20260220_000005_add_commission_rate_to_companies::Migration),
            Box::new(m20260222_000001_add_cart_type_to_carts::Migration),
            Box::new(m20260224_000001_add_cargo_price_to_carts::Migration),
            Box::new(m20260302_000001_add_status_to_cart_items::Migration),
            Box::new(m20260303_000001_create_b2b_credit_transactions::Migration),
            Box::new(m20260303_000002_create_representative_commission_transactions::Migration),
            Box::new(m20260303_000003_remove_representative_columns_from_companies::Migration),
            Box::new(m20260305_000001_add_refund_fields_to_cart_items::Migration),
            Box::new(m20260306_000001_create_return_requests_table::Migration),
            Box::new(m20260311_000001_add_refund_currency_to_cart_items::Migration),
            Box::new(m20260314_000001_add_currency_to_companies::Migration),
            Box::new(m20260416_143557_comment::Migration),
            Box::new(m20260419_215522_add_payment_due_days_to_carts::Migration),
            Box::new(m20260426_000001_add_discount_percentage_to_cart_items::Migration),
            Box::new(m20260428_000001_create_campaigns_table::Migration),
            Box::new(m20260428_000002_create_coupons_table::Migration),
            Box::new(m20260428_000003_create_cart_discounts_table::Migration),
            Box::new(m20260428_000004_create_campaign_usages_table::Migration),
            Box::new(m20260501_130800_add_target_cart_type_to_campaigns::Migration),
            Box::new(m20260523_000001_create_ride_tables::Migration),
            Box::new(m20260526_000001_add_driver_user_id_unique::Migration),
            Box::new(m20260526_000002_add_ride_indexes::Migration),
            Box::new(m20260527_000001_create_ride_fare_configs::Migration),
            Box::new(m20260604_000001_create_locations::Migration),
        ]
    }
}
