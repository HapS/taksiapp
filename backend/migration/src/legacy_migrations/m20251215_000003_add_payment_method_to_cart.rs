use sea_orm_migration::prelude::*;
use sea_orm_migration::prelude::extension::postgres::Type;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Önce enum type'ı oluştur
        manager
            .create_type(
                Type::create()
                    .as_enum(PaymentMethodEnum::Table)
                    .values([
                        PaymentMethodEnum::CreditCard,
                        PaymentMethodEnum::BankTransfer,
                        PaymentMethodEnum::CashOnDelivery,
                        PaymentMethodEnum::Pickup,
                    ])
                    .to_owned(),
            )
            .await?;

        // Sonra kolonu ekle
        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .add_column(
                        ColumnDef::new(Carts::PaymentMethod)
                            .enumeration(PaymentMethodEnum::Table, [
                                PaymentMethodEnum::CreditCard,
                                PaymentMethodEnum::BankTransfer,
                                PaymentMethodEnum::CashOnDelivery,
                                PaymentMethodEnum::Pickup,
                            ])
                            .null()
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .drop_column(Carts::PaymentMethod)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_type(Type::drop().name(PaymentMethodEnum::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Carts {
    Table,
    PaymentMethod,
}

#[derive(DeriveIden)]
enum PaymentMethodEnum {
    #[sea_orm(iden = "payment_method")]
    Table,
    #[sea_orm(iden = "credit_card")]
    CreditCard,
    #[sea_orm(iden = "bank_transfer")]
    BankTransfer,
    #[sea_orm(iden = "cash_on_delivery")]
    CashOnDelivery,
    #[sea_orm(iden = "pickup")]
    Pickup,
}