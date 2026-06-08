use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "exchange_rates")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    // TRY bazlı kurlar
    #[sea_orm(column_type = "Decimal(Some((10, 4)))")]
    pub usd_try: Option<Decimal>,

    #[sea_orm(column_type = "Decimal(Some((10, 4)))")]
    pub eur_try: Option<Decimal>,

    #[sea_orm(column_type = "Decimal(Some((10, 4)))")]
    pub gbp_try: Option<Decimal>,

    #[sea_orm(column_type = "Decimal(Some((10, 4)))")]
    pub chf_try: Option<Decimal>,

    #[sea_orm(column_type = "Decimal(Some((10, 4)))")]
    pub aud_try: Option<Decimal>,

    #[sea_orm(column_type = "Decimal(Some((10, 4)))")]
    pub cad_try: Option<Decimal>,

    #[sea_orm(column_type = "Decimal(Some((10, 4)))")]
    pub azn_try: Option<Decimal>,

    #[sea_orm(column_type = "Decimal(Some((10, 4)))")]
    pub jpy_try: Option<Decimal>,

    // Diğer çapraz kurlar
    #[sea_orm(column_type = "Decimal(Some((10, 4)))")]
    pub eur_usd: Option<Decimal>,

    // Kaynak bilgisi
    pub source: Option<String>, // "tcmb", "exchangerate-api", etc.

    pub created_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Type aliases
pub type ExchangeRate = Entity;
pub type ExchangeRateModel = Model;
pub type ExchangeRateActiveModel = ActiveModel;
#[allow(dead_code)]
pub type ExchangeRateColumn = Column;
