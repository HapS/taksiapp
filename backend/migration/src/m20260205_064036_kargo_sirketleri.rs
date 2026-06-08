use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(KargoSirketleri::Table)
                    .if_not_exists()
                    .col(pk_auto(KargoSirketleri::Id))
                    .col(json_binary(KargoSirketleri::Data))
                    .col(string(KargoSirketleri::Title).not_null())
                    .col(boolean(KargoSirketleri::Publish).not_null().default(true))
                    .col(boolean(KargoSirketleri::Default).not_null().default(false))
                    .col(string(KargoSirketleri::Template).not_null())
                    .col(string(KargoSirketleri::Logo))
                    .col(
                        timestamp(KargoSirketleri::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp())
                            .timestamp_with_time_zone(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(KargoSirketleri::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum KargoSirketleri {
    Table,
    Id,
    Title,
    Publish,
    Default,
    Logo,
    Data,
    Template,
    CreatedAt,
}
