use sea_orm::entity::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "offer_status")]
pub enum OfferStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "accepted")]
    Accepted,
    #[sea_orm(string_value = "rejected")]
    Rejected,
    #[sea_orm(string_value = "timeout")]
    Timeout,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "ride_offers")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub ride_id: i64,
    pub driver_id: i64,
    pub status: OfferStatus,
    pub offer_order: i32,
    pub offered_at: DateTimeWithTimeZone,
    pub responded_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::rides::Entity",
        from = "Column::RideId",
        to = "super::rides::Column::Id"
    )]
    Ride,
    #[sea_orm(
        belongs_to = "super::drivers::Entity",
        from = "Column::DriverId",
        to = "super::drivers::Column::Id"
    )]
    Driver,
}

impl Related<super::rides::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Ride.def()
    }
}

impl Related<super::drivers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Driver.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
