use chrono::NaiveDateTime;
#[cfg(feature = "ssr")]
use diesel::prelude::*;
#[cfg(feature = "ssr")]
use diesel::sqlite::Sqlite;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ssr", derive(Queryable, Selectable))]
#[cfg_attr(feature = "ssr", diesel(table_name = crate::schema::houses))]
pub struct House {
    pub id: i32,
    pub name: String,
    pub score: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ssr", derive(Queryable, Selectable))]
#[cfg_attr(feature = "ssr", diesel(table_name = crate::schema::guests))]
pub struct Guest {
    pub id: i32,
    pub name: String,
    pub house_id: Option<i32>,
    pub personal_score: i32,
    pub is_active: i32,
    pub registered_at: Option<NaiveDateTime>,
    pub character: Option<String>,
}

#[cfg(feature = "ssr")]
#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::guests)]
pub struct NewGuest<'a> {
    pub name: &'a str,
    pub house_id: Option<i32>,
    pub character: Option<&'a str>,
    pub registered_at: Option<chrono::NaiveDateTime>,
    // personal_score and is_active use defaults
}

#[cfg(feature = "ssr")]
#[derive(Queryable, Debug, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::sessions)]
pub struct Session {
    pub id: i32,
    pub guest_id: i32,
    pub token: String,
    pub created_at: NaiveDateTime,
    pub expires_at: Option<NaiveDateTime>,
}

#[cfg(feature = "ssr")]
#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::sessions)]
pub struct NewSession {
    pub guest_id: i32,
    pub token: String,
    // created_at uses default
    // No expires_at (NULL for indefinite)
}

#[cfg(feature = "ssr")]
#[derive(Queryable, Selectable, Debug, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::point_awards)]
#[diesel(check_for_backend(Sqlite))]
pub struct PointAward {
    pub id: i32,
    pub guest_id: Option<i32>,
    pub house_id: Option<i32>,
    pub amount: i32,
    pub reason: String,
    pub awarded_at: NaiveDateTime,
}

#[cfg(feature = "ssr")]
#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::point_awards)]
pub struct NewPointAward {
    pub guest_id: Option<i32>,
    pub house_id: Option<i32>,
    pub amount: i32,
    pub reason: String,
    pub awarded_at: chrono::NaiveDateTime,
}

#[cfg(feature = "ssr")]
#[derive(Queryable, Debug, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::admin_sessions)]
pub struct AdminSession {
    pub id: i32,
    pub token: String,
    pub created_at: NaiveDateTime,
    pub expires_at: Option<NaiveDateTime>,
}

#[cfg(feature = "ssr")]
#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::admin_sessions)]
pub struct NewAdminSession {
    pub token: String,
    // created_at uses default
    // No expires_at (NULL for indefinite)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ssr", derive(Queryable))]
pub struct PointAwardLog {
    pub id: i32,
    pub guest_name: Option<String>,
    pub house_name: Option<String>,
    pub amount: i32,
    pub reason: String,
    pub awarded_at: NaiveDateTime,
}
