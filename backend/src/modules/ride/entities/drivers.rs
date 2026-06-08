use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "drivers")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub user_id: Option<i64>,
    pub full_name: String,
    pub phone: String,
    pub vehicle_plate: String,
    pub vehicle_model: String,
    pub rating: f64,
    pub is_active: bool,
    pub is_online: bool,
    pub current_lat: Option<f64>,
    pub current_lon: Option<f64>,
    pub location_updated_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::rides::Entity")]
    Rides,
    #[sea_orm(has_many = "super::ride_offers::Entity")]
    RideOffers,
}

impl Related<super::rides::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Rides.def()
    }
}

impl Related<super::ride_offers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RideOffers.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
