use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // representative_user_id kolonunu kaldır
        manager
            .alter_table(
                Table::alter()
                    .table(Companies::Table)
                    .drop_column(Companies::RepresentativeUserId)
                    .to_owned(),
            )
            .await?;

        // representative_commission_rate kolonunu kaldır
        manager
            .alter_table(
                Table::alter()
                    .table(Companies::Table)
                    .drop_column(Companies::RepresentativeCommissionRate)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Geri alma: kolonları tekrar ekle
        manager
            .alter_table(
                Table::alter()
                    .table(Companies::Table)
                    .add_column(
                        ColumnDef::new(Companies::RepresentativeUserId)
                            .big_integer()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Companies::Table)
                    .add_column(
                        ColumnDef::new(Companies::RepresentativeCommissionRate)
                            .decimal_len(5, 2)
                            .default(0.00),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Companies {
    Table,
    RepresentativeUserId,
    RepresentativeCommissionRate,
}
