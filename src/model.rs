use chrono::NaiveDateTime;
#[cfg(feature = "ssr")]
use diesel::prelude::*;
#[cfg(feature = "ssr")]
use diesel::sql_types::Text;
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SparseGrid {
    // List of (row, col, char) for non-None cells.
    pub filled: Vec<(usize, usize, char)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SparseState {
    pub filled: Vec<(usize, usize, char)>,
    pub completions: [bool; 7],
}

#[cfg(feature = "ssr")]
#[derive(Queryable, Insertable, Debug)]
#[diesel(table_name = crate::schema::crossword_states)]
pub struct DbCrosswordState {
    pub id: i32,
    pub guest_id: i32,
    #[diesel(sql_type = Text)]
    pub state: String,
    pub updated_at: chrono::NaiveDateTime,
}

#[cfg(feature = "ssr")]
#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::crossword_states)]
pub struct NewDbCrosswordState {
    pub guest_id: i32,
    pub state: String,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CrosswordState {
    // 15 rows x 12 cols; None = unfilled, Some(char) = filled
    pub grid: Vec<Vec<Option<char>>>,
    // Sparse state of grid (for send).
    pub sparse: SparseGrid,
    // Which of the 7 words are completed correctly
    pub completions: [bool; 7],
}

impl CrosswordState {
    pub fn new_full_grid(grid: Vec<Vec<Option<char>>>, completions: [bool; 7]) -> Self {
        let sparse = Self::build_sparse(&grid);
        Self {
            grid,
            sparse,
            completions,
        }
    }

    pub fn to_sparse(&self) -> SparseGrid {
        Self::build_sparse(&self.grid)
    }

    fn build_sparse(grid: &Vec<Vec<Option<char>>>) -> SparseGrid {
        let mut filled = Vec::new();
        for (row, row_vec) in grid.iter().enumerate() {
            for (col, &cell) in row_vec.iter().enumerate() {
                if let Some(c) = cell {
                    filled.push((row, col, c));
                }
            }
        }
        SparseGrid { filled }
    }
}

#[cfg(feature = "ssr")]
impl From<CrosswordState> for String {
    fn from(state: CrosswordState) -> Self {
        let sparse = SparseState {
            filled: state.sparse.filled,
            completions: state.completions,
        };
        serde_json::to_string(&sparse).expect("Failed to serialize sparse state")
    }
}

#[cfg(feature = "ssr")]
impl From<String> for CrosswordState {
    fn from(json: String) -> Self {
        let sparse: SparseState = serde_json::from_str(&json).unwrap_or_default();
        let mut grid = vec![vec![None; 12]; 15];
        for (r, c, ch) in &sparse.filled {
            if *r < 15 && *c < 12 {
                grid[*r][*c] = Some(*ch);
            }
        }
        let sparse_grid = SparseGrid {
            filled: sparse.filled,
        };

        Self {
            grid,
            sparse: sparse_grid,
            completions: sparse.completions,
        }
    }
}

#[cfg(feature = "ssr")]
#[derive(Queryable, Selectable, Debug, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::house_crossword_completions)]
#[diesel(check_for_backend(Sqlite))]
pub struct HouseCrosswordCompletion {
    pub id: i32,
    pub house_id: i32,
    pub word_index: i32,
    pub completed_at: NaiveDateTime,
}

#[cfg(feature = "ssr")]
#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::house_crossword_completions)]
pub struct NewHouseCrosswordCompletion {
    pub house_id: i32,
    pub word_index: i32,
    // completed_at uses default (CURRENT_TIMESTAMP)
}

#[cfg(feature = "ssr")]
#[derive(Queryable, Selectable, Debug, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::voting_status)]
#[diesel(check_for_backend(Sqlite))]
pub struct VotingStatus {
    pub id: i32,
    pub is_open: i32, // 0=closed, 1=open
    pub opened_at: Option<NaiveDateTime>,
    pub closed_at: Option<NaiveDateTime>,
}

#[cfg(feature = "ssr")]
#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::voting_status)]
pub struct NewVotingStatus {
    pub is_open: i32,
    pub opened_at: Option<chrono::NaiveDateTime>,
    pub closed_at: Option<chrono::NaiveDateTime>,
}

#[cfg(feature = "ssr")]
#[derive(Queryable, Selectable, Debug, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::votes)]
#[diesel(check_for_backend(Sqlite))]
pub struct Vote {
    pub id: i32,
    pub voter_id: i32,
    pub first_choice_id: i32,
    pub second_choice_id: i32,
    pub third_choice_id: i32,
    pub submitted_at: NaiveDateTime,
}

#[cfg(feature = "ssr")]
#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::votes)]
pub struct NewVote {
    pub voter_id: i32,
    pub first_choice_id: i32,
    pub second_choice_id: i32,
    pub third_choice_id: i32,
    pub submitted_at: chrono::NaiveDateTime,
}

// Struct for RCV round results (used in app).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcvRound {
    pub round_number: usize,
    pub tallies: Vec<(i32, i32)>, // (guest_id, vote_count)
    pub eliminated: Vec<i32>,     // guest_ids eliminated this round
    pub winner: Option<i32>,      // if declared
}

// Struct for full RCV result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcvResult {
    pub winner_id: Option<i32>,
    pub rounds: Vec<RcvRound>,
}
