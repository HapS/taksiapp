use sea_orm::entity::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "ride_status")]
pub enum RideStatus {
    #[sea_orm(string_value = "searching")]
    Searching,
    #[sea_orm(string_value = "accepted")]
    Accepted,
    #[sea_orm(string_value = "picked_up")]
    PickedUp,
    #[sea_orm(string_value = "completed")]
    Completed,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
    #[sea_orm(string_value = "no_driver")]
    NoDriver,
}

impl RideStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RideStatus::Searching => "searching",
            RideStatus::Accepted => "accepted",
            RideStatus::PickedUp => "picked_up",
            RideStatus::Completed => "completed",
            RideStatus::Cancelled => "cancelled",
            RideStatus::NoDriver => "no_driver",
        }
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "rides")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub user_id: i64,
    pub driver_id: Option<i64>,
    pub status: RideStatus,
    pub pickup_lat: f64,
    pub pickup_lon: f64,
    pub pickup_address: String,
    pub dropoff_lat: f64,
    pub dropoff_lon: f64,
    pub dropoff_address: String,
    pub distance_km: Option<f64>,
    pub duration_sec: Option<i32>,
    pub fare_amount: Option<Decimal>,
    pub requested_at: DateTimeWithTimeZone,
    pub accepted_at: Option<DateTimeWithTimeZone>,
    pub picked_up_at: Option<DateTimeWithTimeZone>,
    pub completed_at: Option<DateTimeWithTimeZone>,
    pub cancelled_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::modules::auth::models::user::Entity",
        from = "Column::UserId",
        to = "crate::modules::auth::models::user::Column::Id"
    )]
    User,
    #[sea_orm(
        belongs_to = "super::drivers::Entity",
        from = "Column::DriverId",
        to = "super::drivers::Column::Id"
    )]
    Driver,
    #[sea_orm(has_many = "super::ride_offers::Entity")]
    RideOffers,
}

impl Related<super::drivers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Driver.def()
    }
}

impl Related<super::ride_offers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RideOffers.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
