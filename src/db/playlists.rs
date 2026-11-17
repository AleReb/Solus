//! Persistent profile-scoped Saved Playlists stored in SQLite (`playlists` + `playlist_items`).
//! Stores durable show/episode intent and playback settings rather than ephemeral CDN links.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

/// Truncate `text` to at most `max_chars` without splitting a word mid-word.
fn truncate_word_boundary(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }
    let budget = max_chars - 1;
    let slice: String = trimmed.chars().take(budget).collect();
    if let Some(last_space) = slice.rfind(char::is_whitespace) {
        if last_space >= budget / 2 {
            return format!("{}…", slice[..last_space].trim_end());
        }
    }
    format!("{}…", slice.trim_end())
}

/// Metadata summary for a saved playlist belonging to a user profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedPlaylistSummary {
    pub id:             i64,
    pub profile_id:     String,
    pub name:           String,
    pub created_at:     i64,
    pub updated_at:     i64,
    pub loop_default:   bool,
    pub item_count:     i64,
    pub preview_titles: Vec<String>,
}

/// Ordered entry inside a saved playlist.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedPlaylistItem {
    pub id:          i64,
    pub playlist_id: i64,
    pub position:    i64,
    pub anime_id:    i64,
    pub mal_id:      Option<i64>,
    pub title:       String,
    pub query:       String,
    pub episode:     u32,
    pub season_year: Option<i64>,
    pub quality:     String,
    pub dub:         bool,
    pub sub_lang:    String,
    pub player:      String,
}

/// Input payload when creating or exporting a playlist entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaylistItemDraft {
    pub anime_id:    i64,
    pub mal_id:      Option<i64>,
    pub title:       String,
    pub query:       String,
    pub episode:     u32,
    pub season_year: Option<i64>,
    pub quality:     String,
    pub dub:         bool,
    pub sub_lang:    String,
    pub player:      String,
}

/// Save a new playlist for `profile_id` with ordered items inside one transaction.
pub async fn save_playlist(
    pool:         &SqlitePool,
    profile_id:   &str,
    name:         &str,
    loop_default: bool,
    items:        &[PlaylistItemDraft],
    now:          i64,
) -> Result<i64> {
    let safe_pid = if profile_id.trim().is_empty() {
        "default"
    } else {
        profile_id.trim()
    };
    let clean_name = name.trim();
    let final_name = if clean_name.is_empty() {
        "Untitled Playlist"
    } else {
        clean_name
    };

    let mut tx = pool.begin().await?;

    let res = sqlx::query(
        "INSERT INTO playlists (profile_id, name, created_at, updated_at, loop_default)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(safe_pid)
    .bind(final_name)
    .bind(now)
    .bind(now)
    .bind(if loop_default { 1i64 } else { 0i64 })
    .execute(&mut *tx)
    .await?;

    let playlist_id = res.last_insert_rowid();

    for (pos, item) in items.iter().enumerate() {
        sqlx::query(
            "INSERT INTO playlist_items (
                playlist_id, position, anime_id, mal_id, title, query, episode,
                season_year, quality, dub, sub_lang, player
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(playlist_id)
        .bind(pos as i64)
        .bind(item.anime_id)
        .bind(item.mal_id)
        .bind(&item.title)
        .bind(&item.query)
        .bind(item.episode as i64)
        .bind(item.season_year)
        .bind(&item.quality)
        .bind(if item.dub { 1i64 } else { 0i64 })
        .bind(&item.sub_lang)
        .bind(&item.player)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(playlist_id)
}

/// List all saved playlists for `profile_id` (most recently updated first).
pub async fn list_playlists_for_profile(
    pool:       &SqlitePool,
    profile_id: &str,
) -> Result<Vec<SavedPlaylistSummary>> {
    let safe_pid = if profile_id.trim().is_empty() {
        "default"
    } else {
        profile_id.trim()
    };

    let rows = sqlx::query(
        "SELECT id, profile_id, name, created_at, updated_at, loop_default
         FROM playlists
         WHERE profile_id = ?
         ORDER BY updated_at DESC, id DESC",
    )
    .bind(safe_pid)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let id: i64 = row.get("id");
        let item_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM playlist_items WHERE playlist_id = ?",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        let item_rows = sqlx::query(
            "SELECT title, episode FROM playlist_items
             WHERE playlist_id = ?
             ORDER BY position ASC
             LIMIT 3",
        )
        .bind(id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let preview_titles: Vec<String> = item_rows
            .into_iter()
            .map(|r| {
                let t: String = r.get("title");
                let ep: i64 = r.get("episode");
                let short = truncate_word_boundary(&t, 18);
                format!("{} E{}", short, ep.max(1))
            })
            .collect();

        out.push(SavedPlaylistSummary {
            id,
            profile_id: row.get("profile_id"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            loop_default: row.get::<i64, _>("loop_default") != 0,
            item_count,
            preview_titles,
        });
    }

    Ok(out)
}

/// Fetch ordered entries for a saved playlist.
pub async fn get_playlist_items(
    pool:        &SqlitePool,
    playlist_id: i64,
) -> Result<Vec<SavedPlaylistItem>> {
    let rows = sqlx::query(
        "SELECT id, playlist_id, position, anime_id, mal_id, title, query, episode,
                season_year, quality, dub, sub_lang, player
         FROM playlist_items
         WHERE playlist_id = ?
         ORDER BY position ASC",
    )
    .bind(playlist_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| SavedPlaylistItem {
            id: r.get("id"),
            playlist_id: r.get("playlist_id"),
            position: r.get("position"),
            anime_id: r.get("anime_id"),
            mal_id: r.get("mal_id"),
            title: r.get("title"),
            query: r.get("query"),
            episode: (r.get::<i64, _>("episode").max(1)) as u32,
            season_year: r.get("season_year"),
            quality: r.get("quality"),
            dub: r.get::<i64, _>("dub") != 0,
            sub_lang: r.get("sub_lang"),
            player: r.get("player"),
        })
        .collect())
}

/// Delete a playlist scoped to `profile_id`, cascading all item rows.
pub async fn delete_playlist(
    pool:        &SqlitePool,
    profile_id:  &str,
    playlist_id: i64,
) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let owned: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM playlists WHERE id = ? AND profile_id = ?",
    )
    .bind(playlist_id)
    .bind(profile_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(pid) = owned else {
        return Ok(false);
    };

    sqlx::query("DELETE FROM playlist_items WHERE playlist_id = ?")
        .bind(pid)
        .execute(&mut *tx)
        .await?;
    let res = sqlx::query("DELETE FROM playlists WHERE id = ? AND profile_id = ?")
        .bind(pid)
        .bind(profile_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(res.rows_affected() > 0)
}

/// Toggle loop_default for a saved playlist belonging to `profile_id`.
#[allow(dead_code)]
pub async fn toggle_playlist_loop(
    pool:        &SqlitePool,
    profile_id:  &str,
    playlist_id: i64,
    now:         i64,
) -> Result<Option<bool>> {
    let cur: Option<i64> = sqlx::query_scalar(
        "SELECT loop_default FROM playlists WHERE id = ? AND profile_id = ?",
    )
    .bind(playlist_id)
    .bind(profile_id)
    .fetch_optional(pool)
    .await?;

    let Some(flag) = cur else {
        return Ok(None);
    };
    let next = if flag == 0 { 1i64 } else { 0i64 };
    sqlx::query("UPDATE playlists SET loop_default = ?, updated_at = ? WHERE id = ? AND profile_id = ?")
        .bind(next)
        .bind(now)
        .bind(playlist_id)
        .bind(profile_id)
        .execute(pool)
        .await?;
    Ok(Some(next != 0))
}

/// Replace the ordered items of an existing playlist owned by `profile_id` inside one transaction.
pub async fn update_playlist_items(
    pool:        &SqlitePool,
    profile_id:  &str,
    playlist_id: i64,
    items:       &[PlaylistItemDraft],
    now:         i64,
) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let owned: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM playlists WHERE id = ? AND profile_id = ?",
    )
    .bind(playlist_id)
    .bind(profile_id)
    .fetch_optional(&mut *tx)
    .await?;

    if owned.is_none() {
        return Ok(false);
    }

    sqlx::query("DELETE FROM playlist_items WHERE playlist_id = ?")
        .bind(playlist_id)
        .execute(&mut *tx)
        .await?;

    for (pos, item) in items.iter().enumerate() {
        sqlx::query(
            "INSERT INTO playlist_items (
                playlist_id, position, anime_id, mal_id, title, query, episode,
                season_year, quality, dub, sub_lang, player
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(playlist_id)
        .bind(pos as i64)
        .bind(item.anime_id)
        .bind(item.mal_id)
        .bind(&item.title)
        .bind(&item.query)
        .bind(item.episode as i64)
        .bind(item.season_year)
        .bind(&item.quality)
        .bind(if item.dub { 1i64 } else { 0i64 })
        .bind(&item.sub_lang)
        .bind(&item.player)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("UPDATE playlists SET updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(playlist_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn draft(anime_id: i64, title: &str, ep: u32, year: Option<i64>) -> PlaylistItemDraft {
        PlaylistItemDraft {
            anime_id,
            mal_id: Some(anime_id + 100),
            title: title.to_string(),
            query: title.to_string(),
            episode: ep,
            season_year: year,
            quality: "1080p".to_string(),
            dub: false,
            sub_lang: "English".to_string(),
            player: "mpv".to_string(),
        }
    }

    #[tokio::test]
    async fn test_playlists_profile_isolation_order_and_cascade() {
        let pool = db::init(":memory:").await.unwrap();

        let id_a = save_playlist(
            &pool,
            "default",
            "Friday Watch Party",
            true,
            &[
                draft(1, "Cowboy Bebop", 5, Some(1998)),
                draft(21, "Frieren", 12, Some(2023)),
            ],
            1_000,
        )
        .await
        .unwrap();

        let id_b = save_playlist(
            &pool,
            "guest",
            "Guest Mix",
            false,
            &[draft(1535, "Death Note", 1, Some(2006))],
            2_000,
        )
        .await
        .unwrap();

        let default_list = list_playlists_for_profile(&pool, "default").await.unwrap();
        assert_eq!(default_list.len(), 1);
        assert_eq!(default_list[0].id, id_a);
        assert_eq!(default_list[0].item_count, 2);
        assert!(default_list[0].loop_default);

        let items = get_playlist_items(&pool, id_a).await.unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Cowboy Bebop");
        assert_eq!(items[0].episode, 5);
        assert_eq!(items[1].title, "Frieren");
        assert_eq!(items[1].episode, 12);

        // Wrong profile cannot delete guest playlist
        assert!(!delete_playlist(&pool, "default", id_b).await.unwrap());
        assert_eq!(list_playlists_for_profile(&pool, "guest").await.unwrap().len(), 1);

        // Deleting profile playlist cascades item rows cleanly
        assert!(delete_playlist(&pool, "default", id_a).await.unwrap());
        assert!(get_playlist_items(&pool, id_a).await.unwrap().is_empty());
        assert!(list_playlists_for_profile(&pool, "default").await.unwrap().is_empty());

        // Toggle loop flag works for owner
        let toggled = toggle_playlist_loop(&pool, "guest", id_b, 3_000).await.unwrap();
        assert_eq!(toggled, Some(true));
    }

    #[tokio::test]
    async fn test_update_playlist_items_reorders_and_persists_edits() {
        let pool = db::init(":memory:").await.unwrap();
        let pid = save_playlist(
            &pool,
            "default",
            "Reordered Night",
            false,
            &[
                draft(1, "Cowboy Bebop", 1, Some(1998)),
                draft(21, "Frieren", 2, Some(2023)),
                draft(1535, "Death Note", 3, Some(2006)),
            ],
            1_000,
        )
        .await
        .unwrap();

        let mut updated = vec![
            draft(1535, "Death Note", 3, Some(2006)),
            draft(1, "Cowboy Bebop", 1, Some(1998)),
        ];
        updated[0].dub = true;
        updated[0].sub_lang = "Arabic".to_string();

        // Wrong profile rejected
        assert!(!update_playlist_items(&pool, "other", pid, &updated, 2_000).await.unwrap());
        // Owner updates atomically
        assert!(update_playlist_items(&pool, "default", pid, &updated, 2_000).await.unwrap());

        let items = get_playlist_items(&pool, pid).await.unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Death Note");
        assert!(items[0].dub);
        assert_eq!(items[0].sub_lang, "Arabic");
        assert_eq!(items[0].position, 0);
        assert_eq!(items[1].title, "Cowboy Bebop");
        assert_eq!(items[1].position, 1);
    }
}
